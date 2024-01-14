use crate::{
    transaction::transaction_request,
    utils::{
        map_cursor_advance_err, map_cursor_advance_until_err,
        map_cursor_advance_until_primary_key_err, map_cursor_delete_err, slice_to_array,
    },
};
use std::marker::PhantomData;
use web_sys::{
    wasm_bindgen::{JsCast, JsValue},
    IdbCursor, IdbCursorDirection, IdbCursorWithValue, IdbRequest,
};

#[cfg(doc)]
use crate::Index;
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

/// Wrapper for [`IDBCursorWithValue`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursorWithValue)
pub struct Cursor<Err> {
    sys: Option<IdbCursor>,
    req: IdbRequest,
    _phantom: PhantomData<Err>,
}

impl<Err> Cursor<Err> {
    pub(crate) async fn from(req: IdbRequest) -> crate::Result<Cursor<Err>, Err> {
        let res = transaction_request(req.clone()).await?;
        let is_already_over = res.is_null();
        let sys = (!is_already_over).then(|| {
            res.dyn_into::<IdbCursor>()
                .expect("Cursor-returning request did not return an IDBCursor")
        });
        Ok(Cursor {
            sys,
            req,
            _phantom: PhantomData,
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

    /// Advance this [`Cursor`] by `count` elements
    ///
    /// Internally, this uses [`IDBCursor::advance`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursor/advance).
    pub async fn advance(&mut self, count: u32) -> crate::Result<(), Err> {
        let Some(sys) = &self.sys else {
            return Err(crate::Error::CursorCompleted);
        };
        sys.advance(count).map_err(map_cursor_advance_err)?;
        if transaction_request(self.req.clone()).await?.is_null() {
            self.sys = None;
        }
        Ok(())
    }

    /// Advance this [`Cursor`] until the provided key
    ///
    /// Note that if this [`Cursor`] was built from an [`Index`], then you need to
    /// encode the [`Array`] yourself.
    ///
    /// Internally, this uses [`IDBCursor::continue`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursor/continue).
    pub async fn advance_until(&mut self, key: &JsValue) -> crate::Result<(), Err> {
        let Some(sys) = &self.sys else {
            return Err(crate::Error::CursorCompleted);
        };
        sys.continue_with_key(key)
            .map_err(map_cursor_advance_until_err)?;
        if transaction_request(self.req.clone()).await?.is_null() {
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
        index_key: &[&JsValue],
        primary_key: &JsValue,
    ) -> crate::Result<(), Err> {
        let Some(sys) = &self.sys else {
            return Err(crate::Error::CursorCompleted);
        };
        sys.continue_primary_key(&slice_to_array(index_key), primary_key)
            .map_err(map_cursor_advance_until_primary_key_err)?;
        if transaction_request(self.req.clone()).await?.is_null() {
            self.sys = None;
        }
        Ok(())
    }

    /// Deletes the value currently pointed by this [`Cursor`]
    ///
    /// Note that this method does not work on key-only cursors over indexes.
    ///
    /// Internally, this uses [`IDBCursor::delete`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursor/delete).
    pub async fn delete(&mut self) -> crate::Result<(), Err> {
        let Some(sys) = &self.sys else {
            return Err(crate::Error::CursorCompleted);
        };
        let req = sys.delete().map_err(map_cursor_delete_err)?;
        transaction_request(req).await?;
        Ok(())
    }
}
