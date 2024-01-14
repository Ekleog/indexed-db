use crate::transaction::transaction_request;
use futures_util::future::{Either, FutureExt};
use std::{
    future::Future,
    marker::PhantomData,
    ops::{Bound, RangeBounds},
};
use web_sys::{
    js_sys::{Array, Number},
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
    /// Note that the unbounded range is not a valid range for IndexedDB.
    ///
    /// Internally, this uses [`IDBObjectStore::count`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/count).
    pub fn count_in(
        &self,
        range: impl RangeBounds<JsValue>,
    ) -> impl Future<Output = Result<usize, crate::Error<Err>>> {
        let range = match make_key_range(range) {
            Ok(range) => range,
            Err(e) => return Either::Left(std::future::ready(Err(e))),
        };
        match self.sys.count_with_key(&range) {
            Ok(count_req) => {
                Either::Right(transaction_request(count_req).map(|res| res.map(map_count_res)))
            }
            Err(e) => Either::Left(std::future::ready(Err(map_count_err(e)))),
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
    /// Note that the unbounded range is not a valid range for IndexedDB.
    /// Unfortunately, the IndexedDb API does not indicate whether an object was actually deleted.
    ///
    /// Internally, this uses [`IDBObjectStore::delete`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/delete).
    pub fn delete_range(
        &self,
        range: impl RangeBounds<JsValue>,
    ) -> impl Future<Output = Result<(), crate::Error<Err>>> {
        let range = match make_key_range(range) {
            Ok(range) => range,
            Err(e) => return Either::Left(std::future::ready(Err(e))),
        };
        match self.sys.delete(&range) {
            Ok(delete_req) => {
                Either::Right(transaction_request(delete_req).map(|res| res.map(|_| ())))
            }
            Err(e) => Either::Left(std::future::ready(Err(map_delete_err(e)))),
        }
    }

    /// Get the object with key `key`
    ///
    /// Internally, this uses [`IDBObjectStore::get`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/get).
    pub fn get(
        &self,
        key: &JsValue,
    ) -> impl Future<Output = Result<Option<JsValue>, crate::Error<Err>>> {
        match self.sys.get(key) {
            Ok(get_req) => {
                Either::Right(transaction_request(get_req).map(|res| res.map(none_if_undefined)))
            }
            Err(err) => Either::Left(std::future::ready(Err(map_get_err(err)))),
        }
    }

    /// Get the first value with a key in `range`, ordered by key
    ///
    /// Note that the unbounded range is not a valid range for IndexedDB.
    ///
    /// Internally, this uses [`IDBObjectStore::get`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/get).
    pub fn get_first_in(
        &self,
        range: impl RangeBounds<JsValue>,
    ) -> impl Future<Output = Result<Option<JsValue>, crate::Error<Err>>> {
        let range = match make_key_range(range) {
            Ok(range) => range,
            Err(e) => return Either::Left(std::future::ready(Err(e))),
        };
        match self.sys.get(&range) {
            Ok(get_req) => {
                Either::Right(transaction_request(get_req).map(|res| res.map(none_if_undefined)))
            }
            Err(e) => Either::Left(std::future::ready(Err(map_get_err(e)))),
        }
    }

    /// Get all the objects in the store, with a maximum number of results of `limit`
    ///
    /// Internally, this uses [`IDBObjectStore::getAll`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/getAll).
    pub fn get_all(
        &self,
        limit: Option<u32>,
    ) -> impl Future<Output = Result<Vec<JsValue>, crate::Error<Err>>> {
        let get_req = match limit {
            None => self.sys.get_all(),
            Some(limit) => self
                .sys
                .get_all_with_key_and_limit(&JsValue::UNDEFINED, limit),
        };
        match get_req {
            Ok(get_req) => {
                Either::Right(transaction_request(get_req).map(|res| res.map(array_to_vec)))
            }
            Err(err) => Either::Left(std::future::ready(Err(map_get_err(err)))),
        }
    }
}

fn none_if_undefined(v: JsValue) -> Option<JsValue> {
    if v.is_undefined() {
        None
    } else {
        Some(v)
    }
}

fn array_to_vec(v: JsValue) -> Vec<JsValue> {
    let array = v
        .dyn_into::<Array>()
        .expect("Value was not of the expected Array type");
    let len = array.length();
    let mut res = Vec::with_capacity(usize::try_from(len).unwrap());
    for i in 0..len {
        res.push(array.get(i));
    }
    res
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
            panic!("Tried counting in an ObjectStore while the transaction was inactive")
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
            panic!("Tried deleting from an ObjectStore while the transaction was inactive")
        }
        Some("DataError") => crate::Error::InvalidKey,
        _ => crate::Error::from_js_value(err),
    }
    .into_user()
}

fn map_get_err<Err>(err: JsValue) -> crate::Error<Err> {
    match error_name!(&err) {
        Some("InvalidStateError") => crate::Error::ObjectStoreWasRemoved,
        Some("TransactionInactiveError") => {
            panic!("Tried getting from an ObjectStore while the transaction was inactive")
        }
        Some("DataError") => crate::Error::InvalidKey,
        _ => crate::Error::from_js_value(err),
    }
    .into_user()
}

fn make_key_range<Err>(range: impl RangeBounds<JsValue>) -> Result<JsValue, crate::Error<Err>> {
    match (range.start_bound(), range.end_bound()) {
        (Bound::Unbounded, Bound::Unbounded) => return Err(crate::Error::InvalidRange),
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
    }
    .map(|k| k.into())
    .map_err(|err| {
        match error_name!(&err) {
            Some("DataError") => crate::Error::InvalidKey,
            _ => crate::Error::from_js_value(err),
        }
        .into_user()
    })
}
