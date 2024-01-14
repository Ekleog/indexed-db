use crate::{
    cursor::CursorDirection,
    transaction::transaction_request,
    utils::{
        array_to_vec, make_key_range, map_add_err, map_count_err, map_count_res, map_delete_err,
        map_get_err, map_open_cursor_err, none_if_undefined, str_slice_to_array,
    },
    Cursor, Index,
};
use futures_util::future::{Either, FutureExt};
use std::{future::Future, marker::PhantomData, ops::RangeBounds};
use web_sys::{
    js_sys::Array, wasm_bindgen::JsValue, IdbCursorDirection, IdbIndexParameters, IdbObjectStore,
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

    /// Build an index over this object store
    ///
    /// Note that this method can only be called from within an `on_upgrade_needed` callback. It returns
    /// a builder, and calling the `create` method on this builder will perform the actual creation.
    ///
    /// Internally, this uses [`IDBObjectStore::createIndex`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/createIndex).
    pub fn build_index<'a>(&self, name: &'a str, key_path: &[&str]) -> IndexBuilder<'a, Err> {
        IndexBuilder {
            store: self.sys.clone(),
            name,
            key_path: str_slice_to_array(key_path),
            options: IdbIndexParameters::new(),
            _phantom: PhantomData,
        }
    }

    /// Delete an index from this object store
    ///
    /// Note that this method can only be called from within an `on_upgrade_needed` callback. It returns
    /// a builder, and calling the `create` method on this builder will perform the actual creation.
    ///
    /// Internally, this uses [`IDBObjectStore::deleteIndex`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/deleteIndex).
    pub fn delete_index(&self, name: &str) -> crate::Result<(), Err> {
        self.sys
            .delete_index(name)
            .map_err(|err| match error_name!(&err) {
                Some("InvalidStateError") => crate::Error::ObjectStoreWasRemoved,
                Some("NotFoundError") => crate::Error::DoesNotExist,
                _ => crate::Error::from_js_value(err),
            })
    }

    /// Add the value `val` to this object store, and return its auto-computed key
    ///
    /// This will error if the key already existed.
    ///
    /// Internally, this uses [`IDBObjectStore::add`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/add).
    pub fn add(&self, value: &JsValue) -> impl Future<Output = crate::Result<JsValue, Err>> {
        match self.sys.add(value) {
            Ok(add_req) => Either::Left(transaction_request::<Err>(add_req)),
            Err(e) => Either::Right(std::future::ready(Err(map_add_err(e)))),
        }
    }

    /// Add the value `val` to this object store, with key `key`
    ///
    /// This will error if the key already existed.
    ///
    /// Internally, this uses [`IDBObjectStore::add`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/add).
    pub fn add_kv(
        &self,
        key: &JsValue,
        value: &JsValue,
    ) -> impl Future<Output = crate::Result<(), Err>> {
        match self.sys.add_with_key(value, key) {
            Ok(add_req) => {
                Either::Left(transaction_request::<Err>(add_req).map(|res| res.map(|_| ())))
            }
            Err(e) => Either::Right(std::future::ready(Err(map_add_err(e)))),
        }
    }

    /// Sets the value `val` to this object store, and return its auto-computed key
    ///
    /// This will overwrite the previous value if the key already existed.
    ///
    /// Internally, this uses [`IDBObjectStore::add`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/add).
    pub fn put(&self, value: &JsValue) -> impl Future<Output = crate::Result<JsValue, Err>> {
        match self.sys.put(value) {
            Ok(add_req) => Either::Left(transaction_request::<Err>(add_req)),
            Err(e) => Either::Right(std::future::ready(Err(map_add_err(e)))),
        }
    }

    /// Add the value `val` to this object store, with key `key`
    ///
    /// This will overwrite the previous value if the key already existed.
    ///
    /// Internally, this uses [`IDBObjectStore::add`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/add).
    pub fn put_kv(
        &self,
        key: &JsValue,
        value: &JsValue,
    ) -> impl Future<Output = crate::Result<(), Err>> {
        match self.sys.put_with_key(value, key) {
            Ok(add_req) => {
                Either::Left(transaction_request::<Err>(add_req).map(|res| res.map(|_| ())))
            }
            Err(e) => Either::Right(std::future::ready(Err(map_add_err(e)))),
        }
    }

    /// Clears this object store
    ///
    /// Internally, this uses [`IDBObjectStore::clear`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/clear).
    pub fn clear(&self) -> impl Future<Output = crate::Result<(), Err>> {
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
            }))),
        }
    }

    /// Counts the number of objects in this store
    ///
    /// Internally, this uses [`IDBObjectStore::count`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/count).
    pub fn count(&self) -> impl Future<Output = crate::Result<usize, Err>> {
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
    pub fn contains(&self, key: &JsValue) -> impl Future<Output = crate::Result<bool, Err>> {
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
    ) -> impl Future<Output = crate::Result<usize, Err>> {
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
    pub fn delete(&self, key: &JsValue) -> impl Future<Output = crate::Result<(), Err>> {
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
    ) -> impl Future<Output = crate::Result<(), Err>> {
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
    pub fn get(&self, key: &JsValue) -> impl Future<Output = crate::Result<Option<JsValue>, Err>> {
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
    ) -> impl Future<Output = crate::Result<Option<JsValue>, Err>> {
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

    /// Get all the objects with a key in the provided range, with a maximum number of results of `limit`
    ///
    /// Internally, this uses [`IDBObjectStore::getAll`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/getAll).
    pub fn get_all_in(
        &self,
        range: impl RangeBounds<JsValue>,
        limit: Option<u32>,
    ) -> impl Future<Output = crate::Result<Vec<JsValue>, Err>> {
        let range = match make_key_range(range) {
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

    /// Get the first existing key in the provided range
    ///
    /// Internally, this uses [`IDBObjectStore::getKey`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/getKey).
    pub fn get_first_key_in(
        &self,
        range: impl RangeBounds<JsValue>,
    ) -> impl Future<Output = crate::Result<Option<JsValue>, Err>> {
        let range = match make_key_range(range) {
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

    /// List all the keys in the object store, with a maximum number of results of `limit`
    ///
    /// Internally, this uses [`IDBObjectStore::getAllKeys`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/getAllKeys).
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

    /// List all the keys in the provided range, with a maximum number of results of `limit`
    ///
    /// Internally, this uses [`IDBObjectStore::getAllKeys`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/getAllKeys).
    pub fn get_all_keys_in(
        &self,
        range: impl RangeBounds<JsValue>,
        limit: Option<u32>,
    ) -> impl Future<Output = crate::Result<Vec<JsValue>, Err>> {
        let range = match make_key_range(range) {
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

    /// Get the [`Index`] with the provided name
    ///
    /// Internally, this uses [`IDBObjectStore::index`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/index).
    pub fn index(&self, name: &str) -> crate::Result<Index<Err>, Err> {
        Ok(Index::from_sys(self.sys.index(name).map_err(
            |err| match error_name!(&err) {
                Some("InvalidStateError") => crate::Error::ObjectStoreWasRemoved,
                Some("NotFoundError") => crate::Error::DoesNotExist,
                _ => crate::Error::from_js_value(err),
            },
        )?))
    }

    /// Open a [`Cursor`] on this object store
    pub fn cursor(&self) -> CursorBuilder<Err> {
        CursorBuilder::from(self.sys.clone())
    }
}

/// Helper to build indexes over an [`ObjectStore`]
pub struct IndexBuilder<'a, Err> {
    store: IdbObjectStore,
    name: &'a str,
    key_path: Array,
    options: IdbIndexParameters,
    _phantom: PhantomData<Err>,
}

impl<'a, Err> IndexBuilder<'a, Err> {
    /// Create the index
    ///
    /// Internally, this uses [`IDBObjectStore::createIndex`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/createIndex).
    pub fn create(self) -> crate::Result<(), Err> {
        self.store
            .create_index_with_str_sequence_and_optional_parameters(
                self.name,
                &self.key_path,
                &self.options,
            )
            .map_err(|err| match error_name!(&err) {
                Some("ConstraintError") => crate::Error::AlreadyExists,
                Some("InvalidAccessError") => crate::Error::InvalidArgument,
                Some("InvalidStateError") => crate::Error::ObjectStoreWasRemoved,
                Some("SyntaxError") => crate::Error::InvalidKey,
                _ => crate::Error::from_js_value(err),
            })
            .map(|_| ())
    }

    /// Mark this index as unique
    ///
    /// Internally, this sets [this property](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/createIndex#unique).
    pub fn unique(mut self) -> Self {
        self.options.unique(true);
        self
    }

    /// Mark this index as multi-entry
    ///
    /// Internally, this sets [this property](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/createIndex#multientry).
    pub fn multi_entry(mut self) -> Self {
        self.options.multi_entry(true);
        self
    }
}

/// Helper to build cursors over [`ObjectStore`]s
pub struct CursorBuilder<Err> {
    store: IdbObjectStore,
    query: JsValue,
    direction: IdbCursorDirection,
    _phantom: PhantomData<Err>,
}

impl<Err> CursorBuilder<Err> {
    pub(crate) fn from(store: IdbObjectStore) -> CursorBuilder<Err> {
        CursorBuilder {
            store,
            query: JsValue::UNDEFINED,
            direction: IdbCursorDirection::Next,
            _phantom: PhantomData,
        }
    }

    /// Open the cursor
    ///
    /// Internally, this uses [`IDBObjectStore::openCursor`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/openCursor).
    pub fn open(self) -> impl Future<Output = crate::Result<Cursor<Err>, Err>> {
        match self
            .store
            .open_cursor_with_range_and_direction(&self.query, self.direction)
        {
            Ok(open_req) => {
                Either::Right(transaction_request(open_req).map(|res| res.map(Cursor::from)))
            }
            Err(err) => Either::Left(std::future::ready(Err(map_open_cursor_err(err)))),
        }
    }
    // TODO: implement `open_key`

    /// Limit the range of the cursor
    ///
    /// Internally, this sets [this property](https://developer.mozilla.org/en-US/docs/Web/API/IDBIndex/openCursor#range).
    pub fn range(mut self, range: impl RangeBounds<JsValue>) -> crate::Result<Self, Err> {
        self.query = make_key_range(range)?;
        Ok(self)
    }

    /// Define the direction of the cursor
    ///
    /// Internally, this sets [this property](https://developer.mozilla.org/en-US/docs/Web/API/IDBIndex/openCursor#direction).
    pub fn direction(mut self, direction: CursorDirection) -> Self {
        self.direction = direction.to_sys();
        self
    }
}
