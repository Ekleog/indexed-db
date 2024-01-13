use std::marker::PhantomData;
use web_sys::{wasm_bindgen::JsValue, IdbObjectStore};

use crate::transaction::transaction_request;

/// Wrapper for [`IDBObjectStore`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore),
/// for use in transactions
#[derive(Debug)]
pub struct ObjectStore<Err> {
    sys: IdbObjectStore,
    _phantom: PhantomData<Err>,
}

impl<Err> ObjectStore<Err> {
    pub(crate) fn from_sys(sys: IdbObjectStore) -> ObjectStore<Err> {
        ObjectStore {
            sys,
            _phantom: PhantomData,
        }
    }

    /// Add the value `val` to this object store, and return its auto-computed key
    ///
    /// Internally, this uses [`IDBObjectStore::add`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/add).
    pub async fn add(&self, value: &JsValue) -> Result<JsValue, crate::Error<Err>> {
        let add_req = self.sys.add(value).map_err(map_add_err)?;
        transaction_request::<Err>(add_req).await
    }
}

fn map_add_err<Err>(err: JsValue) -> crate::Error<Err> {
    match error_name!(&err) {
        Some("ReadOnlyError") => crate::Error::ReadOnly,
        Some("TransactionInactiveError") => {
            panic!("Tried adding to an ObjectStore while the transaction was inactive")
        }
        Some("DataError") => crate::Error::InvalidKey,
        Some("InvalidStateError") => crate::Error::ObjectStoreWasRemoved,
        Some("DataCloneError") => crate::Error::FailedClone,
        Some("ConstraintError") => crate::Error::AlreadyExists,
        _ => crate::Error::from_js_value(err),
    }
    .into_user()
}
