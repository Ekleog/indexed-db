use crate::transaction::transaction_request;
use futures_util::future::{Either, FutureExt};
use std::{
    future::Future,
    marker::PhantomData,
    ops::{Bound, RangeBounds},
};
use web_sys::{
    js_sys::Number,
    wasm_bindgen::{JsCast, JsValue},
    IdbKeyRange, IdbObjectStore,
};

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
    pub fn add(&self, value: &JsValue) -> impl Future<Output = Result<JsValue, crate::Error<Err>>> {
        match self.sys.add(value) {
            Ok(add_req) => Either::Left(transaction_request::<Err>(add_req)),
            Err(e) => Either::Right(std::future::ready(Err(map_add_err(e)))),
        }
    }

    /// Add the value `val` to this object store, with key `key`
    ///
    /// Internally, this uses [`IDBObjectStore::add`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/add).
    pub fn add_kv(
        &self,
        key: &JsValue,
        value: &JsValue,
    ) -> impl Future<Output = Result<(), crate::Error<Err>>> {
        match self.sys.add_with_key(value, key) {
            Ok(add_req) => {
                Either::Left(transaction_request::<Err>(add_req).map(|res| res.map(|_| ())))
            }
            Err(e) => Either::Right(std::future::ready(Err(map_add_err(e)))),
        }
    }

    /// Clears this object store
    ///
    /// Internally, this uses [`IDBObjectStore::clear`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/clear).
    pub fn clear(&self) -> impl Future<Output = Result<(), crate::Error<Err>>> {
        match self.sys.clear() {
            Ok(clear_req) => {
                Either::Left(transaction_request(clear_req).map(|res| res.map(|_| ())))
            }
            Err(err) => Either::Right(std::future::ready(Err(match error_name!(&err) {
                Some("ReadOnlyError") => crate::Error::ReadOnly,
                Some("TransactionInactiveError") => {
                    panic!("Tried clearing an ObjectStore while the transaction was inactive")
                }
                _ => crate::Error::from_js_value(err),
            }
            .into_user()))),
        }
    }

    /// Counts the number of objects in this store
    ///
    /// Internally, this uses [`IDBObjectStore::count`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/count).
    pub fn count(&self) -> impl Future<Output = Result<usize, crate::Error<Err>>> {
        match self.sys.count() {
            Ok(count_req) => {
                Either::Left(transaction_request(count_req).map(|res| res.map(map_count_res)))
            }
            Err(e) => Either::Right(std::future::ready(Err(map_count_err(e)))),
        }
    }

    /// Checks whether the provided key exists in this object store
    ///
    /// Internally, this uses [`IDBObjectStore::count`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/count).
    pub fn contains(&self, key: &JsValue) -> impl Future<Output = Result<bool, crate::Error<Err>>> {
        match self.sys.count_with_key(key) {
            Ok(count_req) => Either::Left(
                transaction_request(count_req).map(|res| res.map(map_count_res).map(|n| n != 0)),
            ),
            Err(e) => Either::Right(std::future::ready(Err(map_count_err(e)))),
        }
    }

    /// Counts the number of objects with a key in `range`
    ///
    /// Internally, this uses [`IDBObjectStore::count`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/count).
    pub fn count_in_range(
        &self,
        range: impl RangeBounds<JsValue>,
    ) -> impl Future<Output = Result<usize, crate::Error<Err>>> {
        let range = match make_key_range(range) {
            Ok(Some(range)) => range,
            Ok(None) => return Either::Left(Either::Left(self.count())),
            Err(e) => return Either::Left(Either::Right(std::future::ready(Err(e)))),
        };
        match self.sys.count_with_key(&range) {
            Ok(count_req) => {
                Either::Right(transaction_request(count_req).map(|res| res.map(map_count_res)))
            }
            Err(e) => Either::Left(Either::Right(std::future::ready(Err(map_count_err(e))))),
        }
    }

    /// Delete the object with key `key`
    ///
    /// Unfortunately, the IndexedDb API does not indicate whether an object was actually deleted.
    ///
    /// Internally, this uses [`IDBObjectStore::delete`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/delete).
    pub fn delete(&self, key: &JsValue) -> impl Future<Output = Result<(), crate::Error<Err>>> {
        match self.sys.delete(key) {
            Ok(delete_req) => {
                Either::Left(transaction_request(delete_req).map(|res| res.map(|_| ())))
            }
            Err(e) => Either::Right(std::future::ready(Err(map_delete_err(e)))),
        }
    }

    /// Delete all the objects with a key in `range`
    ///
    /// Unfortunately, the IndexedDb API does not indicate whether an object was actually deleted.
    ///
    /// Internally, this uses [`IDBObjectStore::delete`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/delete).
    pub fn delete_range(
        &self,
        range: impl RangeBounds<JsValue>,
    ) -> impl Future<Output = Result<(), crate::Error<Err>>> {
        let range = match make_key_range(range) {
            Ok(Some(range)) => range,
            Ok(None) => return Either::Left(Either::Left(self.clear())),
            Err(e) => return Either::Left(Either::Right(std::future::ready(Err(e)))),
        };
        match self.sys.delete(&range) {
            Ok(delete_req) => {
                Either::Right(transaction_request(delete_req).map(|res| res.map(|_| ())))
            }
            Err(e) => Either::Left(Either::Right(std::future::ready(Err(map_delete_err(e))))),
        }
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

fn map_count_res(res: JsValue) -> usize {
    let num = res
        .dyn_into::<Number>()
        .expect("IDBObjectStore::count did not return a Number");
    assert!(
        Number::is_integer(&num),
        "Number of elements in object store is not an integer"
    );
    num.value_of() as usize
}

fn map_count_err<Err>(err: JsValue) -> crate::Error<Err> {
    match error_name!(&err) {
        Some("InvalidStateError") => crate::Error::ObjectStoreWasRemoved,
        Some("TransactionInactiveError") => {
            panic!("Tried adding to an ObjectStore while the transaction was inactive")
        }
        Some("DataError") => crate::Error::InvalidKey,
        _ => crate::Error::from_js_value(err),
    }
    .into_user()
}

fn map_delete_err<Err>(err: JsValue) -> crate::Error<Err> {
    match error_name!(&err) {
        Some("ReadOnlyError") => crate::Error::ReadOnly,
        Some("InvalidStateError") => crate::Error::ObjectStoreWasRemoved,
        Some("TransactionInactiveError") => {
            panic!("Tried adding to an ObjectStore while the transaction was inactive")
        }
        Some("DataError") => crate::Error::InvalidKey,
        _ => crate::Error::from_js_value(err),
    }
    .into_user()
}

fn make_key_range<Err>(
    range: impl RangeBounds<JsValue>,
) -> Result<Option<IdbKeyRange>, crate::Error<Err>> {
    let range = match (range.start_bound(), range.end_bound()) {
        (Bound::Unbounded, Bound::Unbounded) => {
            return Ok(None);
        }
        (Bound::Unbounded, Bound::Included(b)) => IdbKeyRange::upper_bound_with_open(b, false),
        (Bound::Unbounded, Bound::Excluded(b)) => IdbKeyRange::upper_bound_with_open(b, true),
        (Bound::Included(b), Bound::Unbounded) => IdbKeyRange::lower_bound_with_open(b, false),
        (Bound::Excluded(b), Bound::Unbounded) => IdbKeyRange::lower_bound_with_open(b, true),
        (Bound::Included(l), Bound::Included(u)) => {
            IdbKeyRange::bound_with_lower_open_and_upper_open(l, u, false, false)
        }
        (Bound::Included(l), Bound::Excluded(u)) => {
            IdbKeyRange::bound_with_lower_open_and_upper_open(l, u, false, true)
        }
        (Bound::Excluded(l), Bound::Included(u)) => {
            IdbKeyRange::bound_with_lower_open_and_upper_open(l, u, true, false)
        }
        (Bound::Excluded(l), Bound::Excluded(u)) => {
            IdbKeyRange::bound_with_lower_open_and_upper_open(l, u, true, true)
        }
    };
    match range {
        Ok(range) => Ok(Some(range)),
        Err(err) => Err(match error_name!(&err) {
            Some("DataError") => crate::Error::InvalidKey,
            _ => crate::Error::from_js_value(err),
        }
        .into_user()),
    }
}
