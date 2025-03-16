use crate::{
    utils::{err_from_event, str_slice_to_array},
    ObjectStore,
};
use futures_channel::oneshot;
use futures_util::future::{self, Either};
use std::marker::PhantomData;
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
pub struct TransactionBuilder<Err> {
    db: IdbDatabase,
    stores: JsValue,
    mode: IdbTransactionMode,
    _phantom: PhantomData<Err>,
    // TODO: add support for transaction durability when web-sys gets it
}

impl<Err> TransactionBuilder<Err> {
    pub(crate) fn from_names(db: IdbDatabase, names: &[&str]) -> TransactionBuilder<Err> {
        TransactionBuilder {
            db,
            stores: str_slice_to_array(names).into(),
            mode: IdbTransactionMode::Readonly,
            _phantom: PhantomData,
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
    pub async fn run<Ret>(
        self,
        transaction: impl AsyncFnOnce(Transaction<Err>) -> crate::Result<Ret, Err>,
    ) -> crate::Result<Ret, Err>
    {
        let t = self
            .db
            .transaction_with_str_sequence_and_mode(&self.stores, self.mode)
            .map_err(|err| match error_name!(&err) {
                Some("InvalidStateError") => crate::Error::DatabaseIsClosed,
                Some("NotFoundError") => crate::Error::DoesNotExist,
                Some("InvalidAccessError") => crate::Error::InvalidArgument,
                _ => crate::Error::from_js_value(err),
            })?;
        let (tx, rx) = futures_channel::oneshot::channel();
        let fut = {
            let t = t.clone();
            async move {
                let res = transaction(Transaction::from_sys(t.clone())).await;
                let return_value = match &res {
                    Ok(_) => Ok(()),
                    Err(_) => Err(()),
                };
                if let Err(_) = tx.send(res) {
                    // Transaction was cancelled by being dropped, abort it
                    let _ = t.abort();
                }
                return_value
            }
        };

        let fut_pin = std::pin::pin!(fut);
        let fut_pin_with_dyn_dispatch: std::pin::Pin<&mut dyn std::future::Future<Output=Result<(), ()>>> = fut_pin;
        // SAFETY: this is fine as long as we don't return from the current function since
        // the future is pinned here.
        let fut_pin_with_dyn_dispatch_and_static_lifetime: std::pin::Pin<&'static mut dyn std::future::Future<Output=Result<(), ()>>> = unsafe { std::mem::transmute(fut_pin_with_dyn_dispatch) };
        // TODO: A possible safety could be added to ensure the future is not polled after
        // the current function has returned (which would cause UB).
        // The idea would be to check `tx.is_cancelled()` is false before polling the
        // `Pin<&dyn Future> reference on the future (since this check indicates that `rx`
        // still exists, which itself lives in this function just like our pinned future).

        unsafe_jar::run(t, fut_pin_with_dyn_dispatch_and_static_lifetime);
        let res = rx.await;
        if unsafe_jar::POLLED_FORBIDDEN_THING.get() {
            panic!("Transaction blocked without any request under way");
        }
        res.expect("Transaction never completed")
    }
}

pub(crate) async fn transaction_request(req: IdbRequest) -> Result<JsValue, JsValue> {
    // TODO: remove these oneshot-channel in favor of a custom-made atomiccell-based channel.
    // the custom-made channel will not call the waker (because we're handling wakes another way),
    // which'll allow using a panicking context again.
    let (success_tx, success_rx) = oneshot::channel();
    let (error_tx, error_rx) = oneshot::channel();

    // Keep the callbacks alive until execution completed
    let _callbacks = unsafe_jar::add_request(req, success_tx, error_tx);

    let res = match future::select(success_rx, error_rx).await {
        Either::Left((res, _)) => Ok(res.unwrap()),
        Either::Right((res, _)) => Err(res.unwrap()),
    };

    res.map_err(|evt| err_from_event(evt).into()).map(|evt| {
        evt.target()
            .expect("Trying to parse indexed_db::Error from an event that has no target")
            .dyn_into::<web_sys::IdbRequest>()
            .expect(
                "Trying to parse indexed_db::Error from an event that is not from an IDBRequest",
            )
            .result()
            .expect("Failed retrieving the result of successful IDBRequest")
    })
}
