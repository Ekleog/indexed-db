//! All the required to run a transaction

use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use futures_channel::oneshot;
use scoped_tls::scoped_thread_local;
use web_sys::{
    js_sys::Function,
    wasm_bindgen::{closure::Closure, JsCast as _},
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

fn panic_waker() -> Waker {
    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(
            std::ptr::null(),
            &RawWakerVTable::new(clone, wake, wake, drop),
        )
    }
    fn wake(_: *const ()) {
        panic!("IndexedDB transaction tried to await on something other than a request")
    }
    fn drop(_: *const ()) {}
    unsafe {
        Waker::new(
            std::ptr::null(),
            &RawWakerVTable::new(clone, wake, wake, drop),
        )
    }
}

scoped_thread_local!(static CURRENT: Rc<RunnableTransaction<'static>>);

pub fn poll_it(state: &Rc<RunnableTransaction<'static>>) {
    CURRENT.set(&state, || {
        // Poll once, in order to run the transaction until its next await on a request
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            state
                .future
                .borrow_mut()
                .as_mut()
                .poll(&mut Context::from_waker(&panic_waker()))
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
