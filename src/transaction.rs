use std::{
    cell::Cell, future::Future, marker::PhantomData, panic::AssertUnwindSafe, pin::Pin, task::Poll,
};
use web_sys::{
    js_sys::{Array, JsString},
    wasm_bindgen::JsValue,
    IdbDatabase, IdbTransaction, IdbTransactionMode,
};

use crate::ObjectStore;

/// Helper to build a transaction
pub struct TransactionBuilder {
    db: IdbDatabase,
    stores: JsValue,
    mode: IdbTransactionMode,
    // TODO: add support for transaction durability when web-sys gets it
}

impl TransactionBuilder {
    pub(crate) fn from_names(db: IdbDatabase, names: &[&str]) -> TransactionBuilder {
        let stores = Array::new();
        for s in names {
            stores.push(&JsString::from(*s));
        }
        TransactionBuilder {
            db,
            stores: stores.into(),
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
    /// doing so anyway.
    ///
    /// If `transaction` returns an `Ok` value, then the transaction will be committed. If it
    /// returns an `Err` value, then it will be aborted.
    ///
    /// Note that transactions cannot be nested.
    ///
    /// Internally, this uses [`IDBDatabase::transaction`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/transaction).
    pub async fn run<Fun, RetFut, Ret, Err>(
        self,
        transaction: Fun,
    ) -> Result<Ret, crate::Error<Err>>
    where
        Fun: FnOnce(Transaction<Err>) -> RetFut,
        RetFut: Future<Output = Result<Ret, crate::Error<Err>>>,
    {
        let t = self
            .db
            .transaction_with_str_sequence_and_mode(&self.stores, self.mode)
            .map_err(|err| {
                match crate::error::name(&err).as_ref().map(|s| s as &str) {
                    Some("InvalidStateError") => crate::Error::DatabaseIsClosed,
                    Some("NotFoundError") => crate::Error::DoesNotExist,
                    Some("InvalidAccessError") => crate::Error::InvalidArgument,
                    _ => crate::Error::from_js_value(err),
                }
                .into_user()
            })?;
        let fut = transaction(Transaction::from_sys(t.clone()));
        TransactionPoller {
            fut,
            transaction: t,
            pending_requests: 0,
        }
        .await
    }
}

thread_local! {
    static PENDING_REQUESTS: Cell<Option<usize>> = Cell::new(None);
}

pub struct Transaction<Err> {
    sys: IdbTransaction,
    _phantom: PhantomData<Err>,
}

impl<Err> Transaction<Err> {
    fn from_sys(sys: IdbTransaction) -> Transaction<Err> {
        Transaction {
            sys,
            _phantom: PhantomData,
        }
    }

    /// Returns an [`ObjectStore`] that can be used to operate on data in this transaction
    ///
    /// Internally, this uses [`IDBTransaction::object_store`](https://developer.mozilla.org/en-US/docs/Web/API/IDBTransaction/objectStore).
    pub fn object_store(&self, name: &str) -> Result<ObjectStore<Err>, crate::Error<Err>> {
        Ok(ObjectStore::from_sys(self.sys.object_store(name).map_err(
            |err| {
                match crate::error::name(&err).as_ref().map(|s| s as &str) {
                    Some("NotFoundError") => crate::Error::DoesNotExist,
                    _ => crate::Error::from_js_value(err),
                }
                .into_user()
            },
        )?))
    }
}

pin_project_lite::pin_project! {
    struct TransactionPoller<F> {
        #[pin]
        fut: F,
        transaction: IdbTransaction,
        pending_requests: usize,
    }
}

impl<Ret, Err, F> Future for TransactionPoller<F>
where
    F: Future<Output = Result<Ret, crate::Error<Err>>>,
{
    type Output = Result<Ret, crate::Error<Err>>;
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        if PENDING_REQUESTS
            .with(|p| p.replace(Some(*this.pending_requests)))
            .is_some()
        {
            this.transaction
                .abort()
                .expect("Failed aborting transaction upon developer error");
            panic!("Tried to nest transactions");
        }
        let res = match std::panic::catch_unwind(AssertUnwindSafe(|| this.fut.poll(cx))) {
            Ok(res) => res,
            Err(e) => {
                this.transaction
                    .abort()
                    .expect("Failed aborting transaction upon panic");
                std::panic::resume_unwind(e);
            }
        };
        let pending = match PENDING_REQUESTS.with(|p| p.take()) {
            Some(p) => p,
            None => {
                this.transaction
                    .abort()
                    .expect("Failed aborting transaction upon developer error");
                panic!("Tried to nest transactions");
            }
        };
        if pending == 0 {
            this.transaction
                .abort()
                .expect("Failed aborting transaction upon developer error");
            panic!("Transaction blocked without any request under way");
        }
        *this.pending_requests = pending;
        res
    }
}
