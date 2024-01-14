use crate::utils::map_cursor_advance_err;
use std::marker::PhantomData;
use web_sys::{
    wasm_bindgen::{JsCast, JsValue},
    IdbCursorDirection, IdbCursorWithValue,
};

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
    sys: IdbCursorWithValue,
    _phantom: PhantomData<Err>,
}

impl<Err> Cursor<Err> {
    pub(crate) fn from(value: JsValue) -> Cursor<Err> {
        Cursor {
            sys: value
                .dyn_into::<IdbCursorWithValue>()
                .expect("Cursor-returning function did not return an IDBCursorWithValue"),
            _phantom: PhantomData,
        }
    }

    /// Retrieve the value this [`Cursor`] is currently pointing at
    ///
    /// Internally, this uses the [`IDBCursorWithValue::value`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursorWithValue/value) property.
    pub fn value(&self) -> JsValue {
        self.sys
            .value()
            .expect("Failed retrieving value from known-good cursor")
    }

    /// Retrieve the key this [`Cursor`] is currently pointing at
    ///
    /// Internally, this uses the [`IDBCursor::key`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursor/key) property.
    pub fn key(&self) -> JsValue {
        self.sys
            .key()
            .expect("Failed retrieving key from known-good cursor")
    }

    /// Advance this [`Cursor`] by `count` elements
    ///
    /// Panics if `count` is `0`.
    ///
    /// Internally, this uses [`IDBCursor::advance`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursor/advance).
    pub fn advance(&self, count: u32) -> crate::Result<(), Err> {
        self.sys.advance(count).map_err(map_cursor_advance_err)
    }
}

/// Wrapper for [`IDBCursor`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursor)
pub struct KeyCursor<Err> {
    // TODO
    _phantom: PhantomData<Err>,
}
