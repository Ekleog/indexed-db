use crate::{utils::str_slice_to_array, ObjectStore};
use futures_channel::oneshot;
use futures_util::future::{self, Either};
use std::{
    cell::Cell, future::Future, marker::PhantomData, panic::AssertUnwindSafe, pin::Pin, task::Poll,
};
use web_sys::{
    js_sys::Function,
    wasm_bindgen::{closure::Closure, JsCast, JsValue},
    IdbDatabase, IdbRequest, IdbTransaction, IdbTransactionMode,
};

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
    /// doing so anyway.
    ///
    /// If `transaction` returns an `Ok` value, then the transaction will be committed. If it
    /// returns an `Err` value, then it will be aborted.
    ///
    /// Note that transactions cannot be nested.
    ///
    /// Internally, this uses [`IDBDatabase::transaction`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/transaction).
    pub async fn run<Fun, RetFut, Ret>(self, transaction: Fun) -> crate::Result<Ret, Err>
    where
        Fun: FnOnce(Transaction<Err>) -> RetFut,
        RetFut: Future<Output = crate::Result<Ret, Err>>,
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

pub(crate) async fn transaction_request<Err>(req: IdbRequest) -> crate::Result<JsValue, Err> {
    let (success_tx, success_rx) = oneshot::channel();
    let (error_tx, error_rx) = oneshot::channel();

    let on_success = Closure::once(|evt: web_sys::Event| success_tx.send(evt));
    let on_error = Closure::once(|evt: web_sys::Event| error_tx.send(evt));

    req.set_onsuccess(Some(on_success.as_ref().dyn_ref::<Function>().unwrap()));
    req.set_onerror(Some(on_error.as_ref().dyn_ref::<Function>().unwrap()));

    PENDING_REQUESTS.with(|p| {
        p.set(Some(
            p.get()
                .expect("Called transaction methods outside of a transaction")
                .checked_add(1)
                .expect("More than usize::MAX requests ongoing"),
        ))
    });

    let res = match future::select(success_rx, error_rx).await {
        Either::Left((res, _)) => Ok(res.unwrap()),
        Either::Right((res, _)) => Err(res.unwrap()),
    };

    PENDING_REQUESTS.with(|p| {
        p.set(Some(
            p.get()
                .expect("Called transaction methods outside of a transaction")
                .checked_sub(1)
                .expect("Something went wrong with the pending requests accounting"),
        ))
    });

    res.map_err(crate::Error::from_js_event).map(|evt| {
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

pin_project_lite::pin_project! {
    pub(crate) struct TransactionPoller<F> {
        #[pin]
        pub(crate) fut: F,
        pub(crate) transaction: IdbTransaction,
        pub(crate) pending_requests: usize,
    }
}

impl<Ret, Err, F> Future for TransactionPoller<F>
where
    F: Future<Output = crate::Result<Ret, Err>>,
{
    type Output = crate::Result<Ret, Err>;
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
        if let Poll::Ready(res) = res {
            // Ignore the remaining pending requests: transaction will auto-commit when they are done
            PENDING_REQUESTS.with(|p| p.set(None));
            return Poll::Ready(match res {
                Ok(res) => Ok(res), // let transaction auto-commit
                Err(err) => {
                    this.transaction
                        .abort()
                        .expect("Failed aborting transaction upon error");
                    Err(err)
                }
            });
        }
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
        Poll::Pending
    }
}
