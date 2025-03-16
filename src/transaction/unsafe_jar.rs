//! Module used for extracting the `unsafe` implementation details of `transaction`
//!
//! The API exposed from here is entirely safe, and this module's code should be properly audited.

use futures_channel::oneshot;
use futures_util::task::noop_waker;
use scoped_tls::scoped_thread_local;
use std::{
    cell::{Cell, RefCell}, future::Future, ops::DerefMut, rc::Rc, task::{Context, Poll}
};
use web_sys::{
    js_sys::Function,
    wasm_bindgen::{closure::Closure, JsCast},
    IdbRequest, IdbTransaction,
};

pub(crate) struct DeferredRunning<'a> {
    future_done_rx: oneshot::Receiver<AwaitedFutureOutcome>,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> DeferredRunning<'a> {
    pub fn new(
        fut_pinned_with_dyn_dispatch: std::pin::Pin<&'a mut dyn std::future::Future<Output=Result<(), ()>>>,
    ) -> (Self, impl FnOnce(IdbTransaction))
    {
        let (tx, rx) = futures_channel::oneshot::channel();

        // SAFETY: `DeferredRunning` is parametrize with the lifetime of the future, so it
        // cannot outlive it.
        // This also means `DeferredRunning.future_done_rx` shares the same lifetime than the
        // future, and we systematically check this object is still alive (using
        // `tx.is_cancelled()`) before polling the future.
        let fut_pinned_with_dyn_dispatch_and_static_lifetime: std::pin::Pin<&'static mut dyn std::future::Future<Output=Result<(), ()>>> = unsafe { std::mem::transmute(fut_pinned_with_dyn_dispatch) };

        let start_cb = move |transaction| {
            let state = State {
                transaction,
                inflight_requests: Rc::new(Cell::new(0)),
                future_and_done_tx: Rc::new(RefCell::new(
                    (
                        fut_pinned_with_dyn_dispatch_and_static_lifetime,
                        Some(tx),
                    )
                )),
            };
            poll_it(&state);
        };

        let deferred = DeferredRunning {
            future_done_rx: rx,
            _phantom: std::marker::PhantomData,
        };

        (deferred, start_cb)
    }

    pub async fn wait(self) {
        let poll_outcome = self.future_done_rx.await.expect("Transaction never completed");
        if matches!(poll_outcome, AwaitedFutureOutcome::PolledForbiddenThing) {
            panic!("Transaction blocked without any request under way");
        }
    }
}

type PinnedAndDynDispatchedStaticFuture = std::pin::Pin<&'static mut dyn std::future::Future<Output=Result<(), ()>>>;

#[derive(Clone)]
struct State {
    transaction: IdbTransaction,
    // Avoiding the two Rc here with a single big Rc would require the coerce_unsized feature
    inflight_requests: Rc<Cell<usize>>,
    /// Note the static lifetime for the future is a blatant lie here !
    ///
    /// Instead we rely on unsafe transmute to convert future into static one (see `run`
    /// implementation), which is safe since the future is pinned within `run` and the
    /// `State` structure never outlive `run`.
    ///
    /// On top of that, we rely on `oneshot::Sender::is_canceled()` as a safety check
    /// to ensure the future is still alive since the oneshot channel receiver has the
    /// same lifetime scope than the future.
    future_and_done_tx: Rc<RefCell<
        (
            PinnedAndDynDispatchedStaticFuture,
            Option<oneshot::Sender<AwaitedFutureOutcome>>,
        )
    >>,
}

enum AwaitedFutureOutcome {
    Done,
    PolledForbiddenThing,
}

scoped_thread_local!(static CURRENT: State);

fn poll_it(state: &State) {
    CURRENT.set(&state, || {
        let mut borrow = state.future_and_done_tx.borrow_mut();
        let (future, future_done_tx) = borrow.deref_mut();

        // This check is the guarantee that the pinned future hasn't been dropped (since
        // `Sender::is_canceled()` informs us than `rx` is still alive, and we've pinned
        // the future in the same scope than `rx`).
        if future_done_tx.as_ref().expect("Future already polled to completion").is_canceled() {
            // Transaction was cancelled by being dropped, abort it
            let _ = state.transaction.abort();
            return;
        }

        // Poll once, in order to run the transaction until its next await on a request
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            future.as_mut().poll(&mut Context::from_waker(&noop_waker()))
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
                    let _ = future_done_tx.take().expect("Future already polled to completion").send(AwaitedFutureOutcome::PolledForbiddenThing);
                    panic!("Transaction blocked without any request under way");
                }
            }
            Poll::Ready(outcome) => {
                if let Err(_) = future_done_tx.take().expect("Future already polled to completion").send(AwaitedFutureOutcome::Done) {
                    // Transaction was cancelled by being dropped, abort it
                    let _ = state.transaction.abort();
                }
                match outcome {
                    Ok(()) => {
                        // Everything went well! We can ignore this
                    }
                    Err(()) => {
                        // The transaction failed. We should abort it.
                        let _ = state.transaction.abort();
                    }
                }
            }
        }
    });
}

fn send_or_abort<T>(transaction: &IdbTransaction, tx: oneshot::Sender<T>, value: T) {
    if tx.send(value).is_err() {
        // Cancelled transaction by not awaiting on it. Abort the transaction if it has not
        // been aborted already.
        let _ = transaction.abort();
    }
}

pub async fn run<Fut>(
    transaction: IdbTransaction,
    transaction_contents: Fut,
)
where
    Fut: Future<Output = Result<(), ()>>,
{
    let fut_pinned = std::pin::pin!(transaction_contents);

    let (deferred, start_cb) = DeferredRunning::new(fut_pinned);
    start_cb(transaction);
    deferred.wait().await;
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
                send_or_abort(&state.transaction, success_tx, evt);
                poll_it(&state);
            }
        });

        let on_error = Closure::once({
            let state = state.clone();
            move |evt: web_sys::Event| {
                state
                    .inflight_requests
                    .set(state.inflight_requests.get() - 1);
                send_or_abort(&state.transaction, error_tx, evt.clone());
                poll_it(&state);
                // The poll completed without panicking. Make the error not abort the transaction.
                evt.prevent_default();
            }
        });

        req.set_onsuccess(Some(&on_success.as_ref().dyn_ref::<Function>().unwrap()));
        req.set_onerror(Some(&on_error.as_ref().dyn_ref::<Function>().unwrap()));

        // Keep the callbacks alive until they're no longer needed
        (on_success, on_error)
    })
}
