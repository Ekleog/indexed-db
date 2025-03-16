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
    marker::PhantomData,
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

pub struct PolledForbiddenThing;

pub struct RunnableTransaction<'f> {
    transaction: IdbTransaction,
    inflight_requests: Cell<usize>,
    future: RefCell<Pin<Box<dyn 'f + Future<Output = ()>>>>,
    send_polled_forbidden_thing_to: RefCell<Option<oneshot::Sender<PolledForbiddenThing>>>,
}

impl<'f> RunnableTransaction<'f> {
    pub fn new<R, E>(
        transaction: IdbTransaction,
        transaction_contents: impl 'f + Future<Output = Result<R, E>>,
        send_res_to: oneshot::Sender<Result<R, E>>,
        send_polled_forbidden_thing_to: oneshot::Sender<PolledForbiddenThing>,
    ) -> RunnableTransaction<'f>
    where
        R: 'f,
        E: 'f,
    {
        RunnableTransaction {
            transaction: transaction.clone(),
            inflight_requests: Cell::new(0),
            future: RefCell::new(Box::pin(async move {
                let result = transaction_contents.await;
                if result.is_err() {
                    // The transaction failed. We should abort it.
                    let _ = transaction.abort();
                }
                if send_res_to.send(result).is_err() {
                    // Transaction was cancelled by being dropped, abort it
                    let _ = transaction.abort();
                }
            })),
            send_polled_forbidden_thing_to: RefCell::new(Some(send_polled_forbidden_thing_to)),
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
                    let _ = state
                        .send_polled_forbidden_thing_to
                        .borrow_mut()
                        .take()
                        .map(|tx| tx.send(PolledForbiddenThing));
                    panic!("Transaction blocked without any request under way");
                }
            }
            Poll::Ready(()) => {
                // Everything went well! We can ignore this
            }
        }
    });
}

pub struct ScopeCallback<'scope, Args> {
    state: Rc<OnceCell<Weak<RunnableTransaction<'static>>>>,
    maker: Option<Box<dyn 'static + FnOnce(Args) -> RunnableTransaction<'static>>>,

    // 'static itself, but also invariant in 'scope
    // See the definition of std::thread::Scope for why invariance is important.
    _phantom: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

impl<'scope, Args> ScopeCallback<'scope, Args> {
    pub fn set_maker(&mut self, maker: impl 'scope + FnOnce(Args) -> RunnableTransaction<'scope>) {
        // TODO: this is not safe, but can be made safe by adjusting the Rc checks
        let maker: Box<dyn 'scope + FnOnce(Args) -> RunnableTransaction<'scope>> = Box::new(maker);
        let maker: Box<dyn 'static + FnOnce(Args) -> RunnableTransaction<'static>> =
            unsafe { std::mem::transmute(maker) };
        self.maker = Some(maker);
    }

    pub fn run(mut self, args: Args) {
        let maker = self.maker.take().expect("Bug in the indexed-db crate: called ScopeCallback::run without calling ScopeCallback::set_maker first");
        let state: RunnableTransaction<'static> = (maker)(args);
        // SAFETY: We're extending the lifetime of `state` to `'static`.
        // This is safe because the `RunnableTransaction` is not stored anywhere else, and it will be dropped
        // before the end of the enclosing `extend_lifetime_to_scope_and_run` call, at the `Weak::strong_count` check.
        // If it is not, we'll panic and abort the whole process.
        // `'scope` is also guaranteed to outlive `extend_lifetime_to_scope_and_run`.
        let state: RunnableTransaction<'static> = unsafe { std::mem::transmute(state) };
        let state = Rc::new(state);
        let _ = self.state.set(Rc::downgrade(&state));
        poll_it(&state);
    }
}

/// Panics and aborts the whole process if the transaction is not dropped before the end of `must_be_dropped_before`
pub async fn extend_lifetime_to_scope_and_run<'scope, ScopeArgs, ScopeRet>(
    scope: impl AsyncFnOnce(ScopeCallback<'scope, ScopeArgs>) -> ScopeRet,
) -> ScopeRet {
    let state = Rc::new(OnceCell::new());
    let callback = ScopeCallback {
        state: state.clone(),
        maker: None,
        _phantom: PhantomData,
    };
    let result = AssertUnwindSafe((scope)(callback)).catch_unwind().await;
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
    success_tx: oneshot::Sender<web_sys::Event>,
    error_tx: oneshot::Sender<web_sys::Event>,
) -> impl Sized {
    CURRENT.with(move |state| {
        state
            .inflight_requests
            .set(state.inflight_requests.get() + 1);

        let on_success = Closure::once({
            let state = state.clone();
            move |evt: web_sys::Event| {
                state
                    .inflight_requests
                    .set(state.inflight_requests.get() - 1);
                if success_tx.send(evt).is_err() {
                    // Cancelled transaction by not awaiting on it. Abort the transaction if it has not
                    // been aborted already.
                    let _ = state.transaction.abort();
                }
                poll_it(&state);
            }
        });

        let on_error = Closure::once({
            let state = state.clone();
            move |evt: web_sys::Event| {
                evt.prevent_default(); // Do not abort the transaction, we're dealing with it ourselves
                state
                    .inflight_requests
                    .set(state.inflight_requests.get() - 1);
                if error_tx.send(evt).is_err() {
                    // Cancelled transaction by not awaiting on it. Abort the transaction if it has not
                    // been aborted already.
                    let _ = state.transaction.abort();
                }
                poll_it(&state);
            }
        });

        req.set_onsuccess(Some(&on_success.as_ref().dyn_ref::<Function>().unwrap()));
        req.set_onerror(Some(&on_error.as_ref().dyn_ref::<Function>().unwrap()));

        // Keep the callbacks alive until they're no longer needed
        (on_success, on_error)
    })
}
