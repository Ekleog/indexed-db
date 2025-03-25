use crate::{
    transaction::transaction_request,
    utils::{
        make_key_range, map_cursor_advance_err, map_cursor_advance_until_err,
        map_cursor_advance_until_primary_key_err, map_cursor_delete_err, map_cursor_update_err,
        map_open_cursor_err,
    },
};
use futures_util::future::Either;
use std::{future::Future, ops::RangeBounds};
use web_sys::{
    wasm_bindgen::{JsCast, JsValue},
    IdbCursor, IdbCursorDirection, IdbCursorWithValue, IdbIndex, IdbObjectStore, IdbRequest,
};

#[cfg(doc)]
use crate::{Index, ObjectStore};
#[cfg(doc)]
use web_sys::js_sys::Array;

/// The direction for a cursor
pub enum CursorDirection {
    /// Advance one by one
    Next,

    /// Advance, skipping duplicate elements
    NextUnique,

    /// Go back, one by one
    Prev,

    /// Go back, skipping duplicate elements
    PrevUnique,
}

impl CursorDirection {
    pub(crate) fn to_sys(&self) -> IdbCursorDirection {
        match self {
            CursorDirection::Next => IdbCursorDirection::Next,
            CursorDirection::NextUnique => IdbCursorDirection::Nextunique,
            CursorDirection::Prev => IdbCursorDirection::Prev,
            CursorDirection::PrevUnique => IdbCursorDirection::Prevunique,
        }
    }
}

/// Helper to build cursors over [`ObjectStore`]s
pub struct CursorBuilder {
    source: Either<IdbObjectStore, IdbIndex>,
    query: JsValue,
    direction: IdbCursorDirection,
}

impl CursorBuilder {
    pub(crate) fn from_store(store: IdbObjectStore) -> CursorBuilder {
        CursorBuilder {
            source: Either::Left(store),
            query: JsValue::UNDEFINED,
            direction: IdbCursorDirection::Next,
        }
    }

    pub(crate) fn from_index(index: IdbIndex) -> CursorBuilder {
        CursorBuilder {
            source: Either::Right(index),
            query: JsValue::UNDEFINED,
            direction: IdbCursorDirection::Next,
        }
    }

    /// Open the cursor
    ///
    /// Internally, this uses [`IDBObjectStore::openCursor`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/openCursor).
    pub fn open(self) -> impl Future<Output = crate::Result<Cursor>> {
        let req = match self.source {
            Either::Left(store) => {
                store.open_cursor_with_range_and_direction(&self.query, self.direction)
            }
            Either::Right(index) => {
                index.open_cursor_with_range_and_direction(&self.query, self.direction)
            }
        };
        match req {
            Ok(open_req) => Either::Right(Cursor::from(open_req)),
            Err(err) => Either::Left(std::future::ready(Err(map_open_cursor_err(err)))),
        }
    }

    /// Open the cursor as a key-only cursor
    ///
    /// Internally, this uses [`IDBObjectStore::openKeyCursor`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/openKeyCursor).
    pub fn open_key(self) -> impl Future<Output = crate::Result<Cursor>> {
        let req = match self.source {
            Either::Left(store) => {
                store.open_key_cursor_with_range_and_direction(&self.query, self.direction)
            }
            Either::Right(index) => {
                index.open_key_cursor_with_range_and_direction(&self.query, self.direction)
            }
        };
        match req {
            Ok(open_req) => Either::Right(Cursor::from(open_req)),
            Err(err) => Either::Left(std::future::ready(Err(map_open_cursor_err(err)))),
        }
    }

    /// Limit the range of the cursor
    ///
    /// Internally, this sets [this property](https://developer.mozilla.org/en-US/docs/Web/API/IDBIndex/openCursor#range).
    pub fn range(mut self, range: impl RangeBounds<JsValue>) -> crate::Result<Self> {
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

/// Wrapper for [`IDBCursorWithValue`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursorWithValue)
pub struct Cursor {
    sys: Option<IdbCursor>,
    req: IdbRequest,
}

impl Cursor {
    pub(crate) async fn from(req: IdbRequest) -> crate::Result<Cursor> {
        let res = transaction_request(req.clone())
            .await
            .map_err(map_open_cursor_err)?;
        let is_already_over = res.is_null();
        let sys = (!is_already_over).then(|| {
            res.dyn_into::<IdbCursor>()
                .expect("Cursor-returning request did not return an IDBCursor")
        });
        Ok(Cursor {
            sys,
            req,
        })
    }

    /// Retrieve the value this [`Cursor`] is currently pointing at, or `None` if the cursor is completed
    ///
    /// If this cursor was opened as a key-only cursor, then trying to call this method will panic.
    ///
    /// Internally, this uses the [`IDBCursorWithValue::value`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursorWithValue/value) property.
    pub fn value(&self) -> Option<JsValue> {
        self.sys.as_ref().map(|sys| {
            sys.dyn_ref::<IdbCursorWithValue>()
                .expect("Called Cursor::value on a key-only cursor")
                .value()
                .expect("Unable to retrieve value from known-good cursor")
        })
    }

    /// Retrieve the key this [`Cursor`] is currently pointing at, or `None` if the cursor is completed
    ///
    /// Internally, this uses the [`IDBCursor::key`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursor/key) property.
    pub fn key(&self) -> Option<JsValue> {
        self.sys.as_ref().map(|sys| {
            sys.key()
                .expect("Failed retrieving key from known-good cursor")
        })
    }

    /// Retrieve the primary key this [`Cursor`] is currently pointing at, or `None` if the cursor is completed
    ///
    /// Internally, this uses the [`IDBCursor::primaryKey`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursor/key) property.
    pub fn primary_key(&self) -> Option<JsValue> {
        self.sys.as_ref().map(|sys| {
            sys.primary_key()
                .expect("Failed retrieving primary key from known-good cursor")
        })
    }

    /// Advance this [`Cursor`] by `count` elements
    ///
    /// Internally, this uses [`IDBCursor::advance`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursor/advance).
    pub async fn advance(&mut self, count: u32) -> crate::Result<()> {
        let Some(sys) = &self.sys else {
            return Err(crate::Error::CursorCompleted);
        };
        sys.advance(count).map_err(map_cursor_advance_err)?;
        if transaction_request(self.req.clone())
            .await
            .map_err(map_cursor_advance_err)?
            .is_null()
        {
            self.sys = None;
        }
        Ok(())
    }

    /// Advance this [`Cursor`] until the provided key
    ///
    /// Internally, this uses [`IDBCursor::continue`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursor/continue).
    pub async fn advance_until(&mut self, key: &JsValue) -> crate::Result<()> {
        let Some(sys) = &self.sys else {
            return Err(crate::Error::CursorCompleted);
        };
        sys.continue_with_key(key)
            .map_err(map_cursor_advance_until_err)?;
        if transaction_request(self.req.clone())
            .await
            .map_err(map_cursor_advance_until_err)?
            .is_null()
        {
            self.sys = None;
        }
        Ok(())
    }

    /// Advance this [`Cursor`] until the provided primary key
    ///
    /// This is a helper function for cursors built on top of [`Index`]es. It allows for
    /// quick resumption of index walking, faster than [`Cursor::advance_until`] if the
    /// primary key for the wanted element is known.
    ///
    /// Note that this method does not work on cursors over object stores, nor on cursors
    /// which are set with a direction of anything other than `Next` or `Prev`.
    ///
    /// Internally, this uses [`IDBCursor::continuePrimaryKey`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursor/continuePrimaryKey).
    pub async fn advance_until_primary_key(
        &mut self,
        index_key: &JsValue,
        primary_key: &JsValue,
    ) -> crate::Result<()> {
        let Some(sys) = &self.sys else {
            return Err(crate::Error::CursorCompleted);
        };
        sys.continue_primary_key(&index_key, primary_key)
            .map_err(map_cursor_advance_until_primary_key_err)?;
        if transaction_request(self.req.clone())
            .await
            .map_err(map_cursor_advance_until_primary_key_err)?
            .is_null()
        {
            self.sys = None;
        }
        Ok(())
    }

    /// Deletes the value currently pointed by this [`Cursor`]
    ///
    /// Note that this method does not work on key-only cursors over indexes.
    ///
    /// Internally, this uses [`IDBCursor::delete`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursor/delete).
    pub async fn delete(&self) -> crate::Result<()> {
        let Some(sys) = &self.sys else {
            return Err(crate::Error::CursorCompleted);
        };
        let req = sys.delete().map_err(map_cursor_delete_err)?;
        transaction_request(req)
            .await
            .map_err(map_cursor_delete_err)?;
        Ok(())
    }

    /// Update the value currently pointed by this [`Cursor`] to `value`
    ///
    /// Note that this method does not work on key-only cursors over indexes.
    ///
    /// Internally, this uses [`IDBCursor::update`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursor/update).
    pub async fn update(&self, value: &JsValue) -> crate::Result<()> {
        let Some(sys) = &self.sys else {
            return Err(crate::Error::CursorCompleted);
        };
        let req = sys.update(value).map_err(map_cursor_update_err)?;
        transaction_request(req)
            .await
            .map_err(map_cursor_update_err)?;
        Ok(())
    }
}
