use std::future::Future;
use web_sys::{
    js_sys::{Array, JsString},
    wasm_bindgen::JsValue,
    IdbDatabase, IdbTransaction, IdbTransactionMode,
};

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
    /// Internally, this uses [`IDBDatabase::transaction`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/transaction).
    pub async fn run<Fun, RetFut, Ret>(self, _transaction: Fun) -> crate::Result<Ret>
    where
        Fun: FnOnce(Transaction) -> RetFut,
        RetFut: Future<Output = Ret>,
    {
        let t = Transaction::from_sys(
            self.db
                .transaction_with_str_sequence_and_mode(&self.stores, self.mode)
                .map_err(
                    |err| match crate::error::name(&err).as_ref().map(|s| s as &str) {
                        Some("InvalidStateError") => crate::Error::DatabaseIsClosed,
                        Some("NotFoundError") => crate::Error::DoesNotExist,
                        Some("InvalidAccessError") => crate::Error::InvalidArgument,
                        _ => crate::Error::from_js_value(err),
                    },
                )?,
        );
        todo!()
    }
}

pub struct Transaction {
    sys: IdbTransaction,
}

impl Transaction {
    fn from_sys(sys: IdbTransaction) -> Transaction {
        Transaction { sys }
    }
}
