use crate::{
    utils::{err_from_event, str_slice_to_array},
    ObjectStore,
};
use std::{
    cell::RefCell,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};
use web_sys::{
    wasm_bindgen::{JsCast, JsValue},
    IdbDatabase, IdbRequest, IdbTransaction, IdbTransactionMode,
};

pub(crate) mod unsafe_jar;

/// Wrapper for [`IDBTransaction`](https://developer.mozilla.org/en-US/docs/Web/API/IDBTransaction)
#[derive(Debug)]
pub struct Transaction<Err> {
    sys: IdbTransaction,
    _phantom: PhantomData<Err>,
}

impl<Err> Transaction<Err> {
    pub(crate) fn from_sys(sys: IdbTransaction) -> Transaction<Err> {
        Transaction {
            sys,
            _phantom: PhantomData,
        }
    }

    pub(crate) fn as_sys(&self) -> &IdbTransaction {
        &self.sys
    }

    /// Returns an [`ObjectStore`] that can be used to operate on data in this transaction
    ///
    /// Internally, this uses [`IDBTransaction::objectStore`](https://developer.mozilla.org/en-US/docs/Web/API/IDBTransaction/objectStore).
    pub fn object_store(&self, name: &str) -> crate::Result<ObjectStore<Err>, Err> {
        Ok(ObjectStore::from_sys(self.sys.object_store(name).map_err(
            |err| match error_name!(&err) {
                Some("NotFoundError") => crate::Error::DoesNotExist,
                _ => crate::Error::from_js_value(err),
            },
        )?))
    }
}

/// Helper to build a transaction
pub struct TransactionBuilder {
    db: IdbDatabase,
    stores: JsValue,
    mode: IdbTransactionMode,
    // TODO: add support for transaction durability when web-sys gets it
}

impl TransactionBuilder {
    pub(crate) fn from_names(db: IdbDatabase, names: &[&str]) -> TransactionBuilder {
        TransactionBuilder {
            db,
            stores: str_slice_to_array(names).into(),
            mode: IdbTransactionMode::Readonly,
        }
    }

    /// Allow writes in this transaction
    ///
    /// Without this, the transaction will only be allowed reads, and will error upon trying to
    /// write objects.
    pub fn rw(mut self) -> Self {
        self.mode = IdbTransactionMode::Readwrite;
        self
    }

    /// Actually execute the transaction
    ///
    /// The `transaction` argument defines what will be run in the transaction. Note that due to
    /// limitations of the IndexedDb API, the future returned by `transaction` cannot call `.await`
    /// on any future except the ones provided by the [`Transaction`] itself. This function will
    /// do its best to detect these cases to abort the transaction and panic, but you should avoid
    /// doing so anyway. Note also that these errors are not recoverable: even if wasm32 were not
    /// having `panic=abort`, once there is such a panic no `indexed-db` functions will work any
    /// longer.
    ///
    /// If `transaction` returns an `Ok` value, then the transaction will be committed. If it
    /// returns an `Err` value, then it will be aborted.
    ///
    /// Note that you should avoid sending requests that you do not await. If you do, it is hard
    /// to say whether the transaction will commit or abort, due to both the IndexedDB and the
    /// `wasm-bindgen` semantics.
    ///
    /// Note that transactions cannot be nested.
    ///
    /// Internally, this uses [`IDBDatabase::transaction`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/transaction).
    // For more details of what will happen if one does not await:
    // - If the `Closure` from `transaction_request` is not dropped yet, then the error will be
    //   explicitly ignored, and thus transaction will commit.
    // - If the `Closure` from `transaction_request` has already been dropped, then the callback
    //   will panic. Most likely this will lead to the transaction aborting, but this is an
    //   untested and unsupported code path.
    pub async fn run<Ret, Err>(
        self,
        transaction: impl AsyncFnOnce(Transaction<Err>) -> crate::Result<Ret, Err>,
    ) -> crate::Result<Ret, Err> {
        let t = self
            .db
            .transaction_with_str_sequence_and_mode(&self.stores, self.mode)
            .map_err(|err| match error_name!(&err) {
                Some("InvalidStateError") => crate::Error::DatabaseIsClosed,
                Some("NotFoundError") => crate::Error::DoesNotExist,
                Some("InvalidAccessError") => crate::Error::InvalidArgument,
                _ => crate::Error::from_js_value(err),
            })?;
        let result = RefCell::new(None);
        let result = &result;
        let (finished_tx, finished_rx) = futures_channel::oneshot::channel();
        unsafe_jar::extend_lifetime_to_scope_and_run(
            Box::new(move |()| {
                unsafe_jar::RunnableTransaction::new(
                    t.clone(),
                    transaction(Transaction::from_sys(t)),
                    result,
                    finished_tx,
                )
            }),
            async move |s| {
                s.run(());
                let _ = finished_rx.await;
                let result = result
                    .borrow_mut()
                    .take()
                    .expect("Transaction finished without setting result");
                match result {
                    unsafe_jar::TransactionResult::PolledForbiddenThing => {
                        panic!("Transaction blocked without any request under way")
                    }
                    unsafe_jar::TransactionResult::Done(r) => r,
                }
            },
        )
        .await
    }
}

struct FakeFuture<'a, T> {
    watching: &'a RefCell<Option<T>>,
}

impl<'a, T> FakeFuture<'a, T> {
    fn new(watching: &'a RefCell<Option<T>>) -> FakeFuture<'a, T> {
        FakeFuture { watching }
    }
}

impl<'a, T> Future for FakeFuture<'a, T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<T> {
        // Don't do this at home! This only works thanks to our unsafe jar polling regardless of the waker
        match self.watching.borrow_mut().take() {
            None => Poll::Pending,
            Some(r) => Poll::Ready(r),
        }
    }
}

pub(crate) async fn transaction_request(req: IdbRequest) -> Result<JsValue, JsValue> {
    // TODO: our custom-made channel will not call the waker (because we're handling wakes another way),
    // so we can use a panicking context again.
    let result = Rc::new(RefCell::new(None));

    // Keep the callbacks alive until execution completed
    let _callbacks = unsafe_jar::add_request(req, &result);

    match FakeFuture::new(&result).await {
        Ok(evt) => {
            let result = evt.target()
                .expect("Trying to parse indexed_db::Error from an event that has no target")
                .dyn_into::<web_sys::IdbRequest>()
                .expect("Trying to parse indexed_db::Error from an event that is not from an IDBRequest")
                .result()
                .expect("Failed retrieving the result of successful IDBRequest");
            Ok(result)
        }
        Err(evt) => Err(err_from_event(evt).into()),
    }
}
