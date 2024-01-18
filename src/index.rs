use crate::{
    transaction::transaction_request,
    utils::{
        array_to_vec, make_key_range_from_slice, map_count_err, map_count_res, map_get_err,
        none_if_undefined, slice_to_array,
    },
};
use futures_util::future::{Either, FutureExt};
use std::{future::Future, marker::PhantomData, ops::RangeBounds};
use web_sys::{wasm_bindgen::JsValue, IdbIndex};

/// Wrapper for [`IDBIndex`](https://developer.mozilla.org/en-US/docs/Web/API/IDBIndex),
/// for use in transactions
pub struct Index<Err> {
    sys: IdbIndex,
    _phantom: PhantomData<Err>,
}

impl<Err> Index<Err> {
    pub(crate) fn from_sys(sys: IdbIndex) -> Index<Err> {
        Index {
            sys,
            _phantom: PhantomData,
        }
    }

    /// Checks whether the provided key (for this index) already exists
    ///
    /// Internally, this uses [`IDBIndex::count`](https://developer.mozilla.org/en-US/docs/Web/API/IDBIndex/count).
    pub fn contains(&self, key: &[&JsValue]) -> impl Future<Output = crate::Result<bool, Err>> {
        match self.sys.count_with_key(&slice_to_array(key)) {
            Ok(count_req) => Either::Right(
                transaction_request(count_req).map(|res| res.map(map_count_res).map(|n| n != 0)),
            ),
            Err(e) => Either::Left(std::future::ready(Err(map_count_err(e)))),
        }
    }

    /// Count all the keys (for this index) in the provided range
    ///
    /// Internally, this uses [`IDBIndex::count`](https://developer.mozilla.org/en-US/docs/Web/API/IDBIndex/count).
    pub fn count_in<'a>(
        &self,
        range: impl RangeBounds<[&'a JsValue]>,
    ) -> impl Future<Output = crate::Result<usize, Err>> {
        let range = match make_key_range_from_slice(range) {
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

    /// Get the object with key `key` for this index
    ///
    /// Internally, this uses [`IDBIndex::get`](https://developer.mozilla.org/en-US/docs/Web/API/IDBIndex/get).
    pub fn get(
        &self,
        key: &[&JsValue],
    ) -> impl Future<Output = crate::Result<Option<JsValue>, Err>> {
        match self.sys.get(&slice_to_array(key)) {
            Ok(get_req) => {
                Either::Right(transaction_request(get_req).map(|res| res.map(none_if_undefined)))
            }
            Err(err) => Either::Left(std::future::ready(Err(map_get_err(err)))),
        }
    }

    /// Get the first value with a key (for this index) in `range`, ordered by key (for this index)
    ///
    /// Note that the unbounded range is not a valid range for IndexedDB.
    ///
    /// Internally, this uses [`IDBIndex::get`](https://developer.mozilla.org/en-US/docs/Web/API/IDBIndex/get).
    pub fn get_first_in<'a>(
        &self,
        range: impl RangeBounds<[&'a JsValue]>,
    ) -> impl Future<Output = crate::Result<Option<JsValue>, Err>> {
        let range = match make_key_range_from_slice(range) {
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

    /// Get all the objects in the store, ordered by this index, with a maximum number of results of `limit`
    ///
    /// Internally, this uses [`IDBIndex::getAll`](https://developer.mozilla.org/en-US/docs/Web/API/IDBIndex/getAll).
    pub fn get_all(
        &self,
        limit: Option<u32>,
    ) -> impl Future<Output = crate::Result<Vec<JsValue>, Err>> {
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

    /// Get all the objects with a key (for this index) in the provided range, with a maximum number of
    /// results of `limit`, ordered by this index
    ///
    /// Internally, this uses [`IDBIndex::getAll`](https://developer.mozilla.org/en-US/docs/Web/API/IDBIndex/getAll).
    pub fn get_all_in<'a>(
        &self,
        range: impl RangeBounds<[&'a JsValue]>,
        limit: Option<u32>,
    ) -> impl Future<Output = crate::Result<Vec<JsValue>, Err>> {
        let range = match make_key_range_from_slice(range) {
            Ok(range) => range,
            Err(e) => return Either::Left(std::future::ready(Err(e))),
        };
        let get_req = match limit {
            None => self.sys.get_all_with_key(&range),
            Some(limit) => self.sys.get_all_with_key_and_limit(&range, limit),
        };
        match get_req {
            Ok(get_req) => {
                Either::Right(transaction_request(get_req).map(|res| res.map(array_to_vec)))
            }
            Err(err) => Either::Left(std::future::ready(Err(map_get_err(err)))),
        }
    }

    /// Get the first existing key (for this index) in the provided range
    ///
    /// Internally, this uses [`IDBIndex::getKey`](https://developer.mozilla.org/en-US/docs/Web/API/IDBIndex/getKey).
    pub fn get_first_key_in<'a>(
        &self,
        range: impl RangeBounds<[&'a JsValue]>,
    ) -> impl Future<Output = crate::Result<Option<JsValue>, Err>> {
        let range = match make_key_range_from_slice(range) {
            Ok(range) => range,
            Err(e) => return Either::Left(std::future::ready(Err(e))),
        };
        match self.sys.get_key(&range) {
            Ok(get_req) => {
                Either::Right(transaction_request(get_req).map(|res| res.map(none_if_undefined)))
            }
            Err(err) => Either::Left(std::future::ready(Err(map_get_err(err)))),
        }
    }

    /// List all the keys (for this index) in the object store, with a maximum number of results of `limit`, ordered by this index
    ///
    /// Internally, this uses [`IDBIndex::getAllKeys`](https://developer.mozilla.org/en-US/docs/Web/API/IDBIndex/getAllKeys).
    pub fn get_all_keys(
        &self,
        limit: Option<u32>,
    ) -> impl Future<Output = crate::Result<Vec<JsValue>, Err>> {
        let get_req = match limit {
            None => self.sys.get_all_keys(),
            Some(limit) => self
                .sys
                .get_all_keys_with_key_and_limit(&JsValue::UNDEFINED, limit),
        };
        match get_req {
            Ok(get_req) => {
                Either::Right(transaction_request(get_req).map(|res| res.map(array_to_vec)))
            }
            Err(err) => Either::Left(std::future::ready(Err(map_get_err(err)))),
        }
    }

    /// List all the keys (for this index) in the provided range, with a maximum number of results of `limit`,
    /// ordered by this index
    ///
    /// Internally, this uses [`IDBIndex::getAllKeys`](https://developer.mozilla.org/en-US/docs/Web/API/IDBIndex/getAllKeys).
    pub fn get_all_keys_in<'a>(
        &self,
        range: impl RangeBounds<[&'a JsValue]>,
        limit: Option<u32>,
    ) -> impl Future<Output = crate::Result<Vec<JsValue>, Err>> {
        let range = match make_key_range_from_slice(range) {
            Ok(range) => range,
            Err(e) => return Either::Left(std::future::ready(Err(e))),
        };
        let get_req = match limit {
            None => self.sys.get_all_keys_with_key(&range),
            Some(limit) => self.sys.get_all_keys_with_key_and_limit(&range, limit),
        };
        match get_req {
            Ok(get_req) => {
                Either::Right(transaction_request(get_req).map(|res| res.map(array_to_vec)))
            }
            Err(err) => Either::Left(std::future::ready(Err(map_get_err(err)))),
        }
    }

    // TODO: openCursor
    // TODO: openKeyCursor
}
