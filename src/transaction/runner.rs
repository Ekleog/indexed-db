//! All the required to run a transaction
//!
//! Originally, this module was used for extracting the `unsafe` implementation details of `transaction`.
//! Since then, all the code here has been made safe.
//! However, it is possible that in the future, we'll need more unsafe code, in which case it would likely
//! have to come here.
//!
//! The API exposed from here is entirely safe, and this module's code should be properly audited.

use futures_channel::oneshot;
use futures_util::task::noop_waker;
use scoped_tls::scoped_thread_local;
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};
use web_sys::{
    js_sys::Function,
    wasm_bindgen::{closure::Closure, JsCast},
    IdbRequest, IdbTransaction,
};

pub struct PolledForbiddenThing;

pub struct RunnableTransaction {
    transaction: IdbTransaction,
    inflight_requests: Cell<usize>,
    future: RefCell<Pin<Box<dyn 'static + Future<Output = ()>>>>,
    send_polled_forbidden_thing_to: RefCell<Option<oneshot::Sender<PolledForbiddenThing>>>,
}

impl RunnableTransaction {
    pub fn new<R, E>(
        transaction: IdbTransaction,
        transaction_contents: impl 'static + Future<Output = Result<R, E>>,
        send_res_to: oneshot::Sender<Result<R, E>>,
        send_polled_forbidden_thing_to: oneshot::Sender<PolledForbiddenThing>,
    ) -> RunnableTransaction
    where
        R: 'static,
        E: 'static,
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

scoped_thread_local!(static CURRENT: Rc<RunnableTransaction>);

fn poll_it(state: &Rc<RunnableTransaction>) {
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

pub fn run(state: RunnableTransaction) {
    poll_it(&Rc::new(state));
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
