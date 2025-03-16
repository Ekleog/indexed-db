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

#[derive(Clone)]
struct State {
    transaction: IdbTransaction,
    // Avoiding the two Rc here with a single big Rc would require the coerce_unsized feature
    inflight_requests: Rc<Cell<usize>>,
    future: Rc<RefCell<Pin<Box<dyn 'static + Future<Output = Result<(), ()>>>>>>,
}

scoped_thread_local!(static CURRENT: State);
thread_local!(pub(crate) static POLLED_FORBIDDEN_THING: Cell<bool> = Cell::new(false));

fn poll_it(state: &State) {
    CURRENT.set(&state, || {
        // Poll once, in order to run the transaction until its next await on a request
        let mut transaction_fut = state.future.borrow_mut();
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            transaction_fut
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
                    POLLED_FORBIDDEN_THING.set(true);
                    panic!("Transaction blocked without any request under way");
                }
            }
            Poll::Ready(Ok(())) => {
                // Everything went well! We can ignore this
            }
            Poll::Ready(Err(())) => {
                // The transaction failed. We should abort it.
                let _ = state.transaction.abort();
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

pub fn run<Fut>(transaction: IdbTransaction, transaction_contents: Fut)
where
    Fut: 'static + Future<Output = Result<(), ()>>,
{
    let state = State {
        transaction,
        inflight_requests: Rc::new(Cell::new(0)),
        future: Rc::new(RefCell::new(Box::pin(transaction_contents))),
    };
    poll_it(&state as _);
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
