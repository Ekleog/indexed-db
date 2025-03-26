//! All the required to run a transaction
//!
//! Originally, this module was used for extracting the `unsafe` implementation details of `transaction`.
//! Since then, all the code here has been made safe.
//! However, it is possible that in the future, we'll need more unsafe code, in which case it would likely
//! have to come here.
//!
//! The API exposed from here is entirely safe, and this module's code should be properly audited.

use futures_channel::oneshot;
use futures_util::{task::noop_waker, FutureExt};
use scoped_tls::scoped_thread_local;
use std::{
    cell::{Cell, OnceCell, RefCell},
    future::Future,
    panic::AssertUnwindSafe,
    pin::Pin,
    rc::{Rc, Weak},
    task::{Context, Poll},
};
use web_sys::{
    js_sys::Function,
    wasm_bindgen::{closure::Closure, JsCast},
    IdbRequest, IdbTransaction,
};

pub enum TransactionResult<R> {
    PolledForbiddenThing,
    Done(R),
}

pub struct RunnableTransaction<'f> {
    transaction: IdbTransaction,
    inflight_requests: Cell<usize>,
    future: RefCell<Pin<Box<dyn 'f + Future<Output = ()>>>>,
    polled_forbidden_thing: Box<dyn 'f + Fn()>,
    finished: RefCell<Option<oneshot::Sender<()>>>,
}

impl<'f> RunnableTransaction<'f> {
    pub fn new<R, E>(
        transaction: IdbTransaction,
        transaction_contents: impl 'f + Future<Output = Result<R, E>>,
        result: &'f RefCell<Option<TransactionResult<Result<R, E>>>>,
        finished: oneshot::Sender<()>,
    ) -> RunnableTransaction<'f>
    where
        R: 'f,
        E: 'f,
    {
        RunnableTransaction {
            transaction: transaction.clone(),
            inflight_requests: Cell::new(0),
            future: RefCell::new(Box::pin(async move {
                let transaction_result = transaction_contents.await;
                if transaction_result.is_err() {
                    // The transaction failed. We should abort it.
                    let _ = transaction.abort();
                }
                assert!(
                    result
                        .replace(Some(TransactionResult::Done(transaction_result)))
                        .is_none(),
                    "Transaction completed multiple times",
                );
            })),
            polled_forbidden_thing: Box::new(move || {
                *result.borrow_mut() = Some(TransactionResult::PolledForbiddenThing);
            }),
            finished: RefCell::new(Some(finished)),
        }
    }
}

scoped_thread_local!(static CURRENT: Rc<RunnableTransaction<'static>>);

fn poll_it(state: &Rc<RunnableTransaction<'static>>) {
    CURRENT.set(&state, || {
        // Poll once, in order to run the transaction until its next await on a request
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            state
                .future
                .borrow_mut()
                .as_mut()
                .poll(&mut Context::from_waker(&noop_waker()))
        }));

        // Try catching the panic and aborting. This currently does not work in wasm due to panic=abort, but will
        // hopefully work some day. The transaction _should_ auto-abort if the wasm module aborts, so hopefully we're
        // fine around there.
        let res = match res {
            Ok(res) => res,
            Err(err) => {
                // The poll panicked, abort the transaction
                let _ = state.transaction.abort();
                std::panic::resume_unwind(err);
            }
        };

        // Finally, check the poll result
        match res {
            Poll::Pending => {
                // Still some work to do. Is there at least one request in flight?
                if state.inflight_requests.get() == 0 {
                    // Returned `Pending` despite no request being inflight. This means there was
                    // an `await` on something other than transaction requests. Abort in order to
                    // avoid the default auto-commit behavior.
                    let _ = state.transaction.abort();
                    let _ = (state.polled_forbidden_thing)();
                }
            }
            Poll::Ready(()) => {
                // Everything went well! Just signal that we're done
                let finished = state
                    .finished
                    .borrow_mut()
                    .take()
                    .expect("Transaction finished multiple times");
                if finished.send(()).is_err() {
                    // Transaction aborted by not awaiting on it
                    let _ = state.transaction.abort();
                    return;
                }
            }
        }
    });
}

struct DropFlag(Rc<Cell<bool>>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

pub struct ScopeCallback<Args> {
    state: Rc<OnceCell<Weak<RunnableTransaction<'static>>>>,
    _dropped: DropFlag,
    maker: Box<dyn 'static + FnOnce(Args) -> RunnableTransaction<'static>>,
}

impl<Args> ScopeCallback<Args> {
    pub fn run(self, args: Args) {
        let made_state = Rc::new((self.maker)(args));
        let _ = self.state.set(Rc::downgrade(&made_state));
        poll_it(&made_state);
    }
}

/// Panics and aborts the whole process if the transaction is not dropped before the end of `scope`
pub async fn extend_lifetime_to_scope_and_run<'scope, MakerArgs, ScopeRet>(
    maker: Box<dyn 'scope + FnOnce(MakerArgs) -> RunnableTransaction<'scope>>,
    scope: impl 'scope + AsyncFnOnce(ScopeCallback<MakerArgs>) -> ScopeRet,
) -> ScopeRet {
    // SAFETY: We're extending the lifetime of `maker` as well as its return value to `'static`.
    // This is safe because the `RunnableTransaction` is not stored anywhere else, and it will be dropped
    // before the end of the enclosing `extend_lifetime_to_scope_and_run` call, at the `Weak::strong_count` check.
    // If it is not, we'll panic and abort the whole process.
    // `'scope` is also guaranteed to outlive `extend_lifetime_to_scope_and_run`.
    // Finally, `maker` itself is guaranteed to not escape `'scope` because it can only be consumed by `run`,
    // and the `ScopeCallback` itself is guaranteed to not escape `'scope` thanks to the check on `dropped`.
    let maker: Box<dyn 'static + FnOnce(MakerArgs) -> RunnableTransaction<'static>> =
        unsafe { std::mem::transmute(maker) };

    let state = Rc::new(OnceCell::new());
    let dropped = Rc::new(Cell::new(false));
    let callback = ScopeCallback {
        state: state.clone(),
        _dropped: DropFlag(dropped.clone()),
        maker,
    };
    let result = AssertUnwindSafe((scope)(callback)).catch_unwind().await;
    if !dropped.get() {
        let _ = std::panic::catch_unwind(|| {
            panic!("Bug in the indexed-db crate: the ScopeCallback was not consumed before the end of its logical lifetime")
        });
        std::process::abort();
    }
    if let Some(state) = state.get() {
        if Weak::strong_count(&state) != 0 {
            // Make sure that regardless of what the user could be doing, if we're overextending the lifetime we'll panic and abort
            //
            // Note: we know this won't spuriously hit because:
            // - we're using `Rc`, so every `RunnableTransaction` operation is single-thread anyway
            // - when the scope completes, at least `result_rx` or `polled_forbidden_thing_rx` will have resolved
            // - either of these channels being written to, means that the `RunnableTransaction` has been dropped
            // Point 2 is enforced outside of the unsafe jar, but it's fine considering it will only result in a spurious panic/abort
            let _ = std::panic::catch_unwind(|| {
                panic!("Bug in the indexed-db crate: the transaction was not dropped before the end of its lifetime")
            });
            std::process::abort();
        }
    }
    match result {
        Ok(result) => result,
        Err(err) => std::panic::resume_unwind(err),
    }
}

pub fn add_request(
    req: IdbRequest,
    result: &Rc<RefCell<Option<Result<web_sys::Event, web_sys::Event>>>>,
) -> impl Sized {
    CURRENT.with(move |state| {
        state
            .inflight_requests
            .set(state.inflight_requests.get() + 1);

        let on_success = Closure::once({
            let state = state.clone();
            let result = result.clone();
            move |evt: web_sys::Event| {
                state
                    .inflight_requests
                    .set(state.inflight_requests.get() - 1);
                assert!(result.replace(Some(Ok(evt))).is_none());
                poll_it(&state);
            }
        });

        let on_error = Closure::once({
            let state = state.clone();
            let result = result.clone();
            move |evt: web_sys::Event| {
                evt.prevent_default(); // Do not abort the transaction, we're dealing with it ourselves
                state
                    .inflight_requests
                    .set(state.inflight_requests.get() - 1);
                assert!(result.replace(Some(Err(evt))).is_none());
                poll_it(&state);
            }
        });

        req.set_onsuccess(Some(&on_success.as_ref().dyn_ref::<Function>().unwrap()));
        req.set_onerror(Some(&on_error.as_ref().dyn_ref::<Function>().unwrap()));

        // Keep the callbacks alive until they're no longer needed
        (on_success, on_error)
    })
}
