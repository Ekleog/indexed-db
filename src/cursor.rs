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
    _sys: IdbCursorWithValue,
    _phantom: PhantomData<Err>,
}

impl<Err> Cursor<Err> {
    pub(crate) fn from(value: JsValue) -> Cursor<Err> {
        Cursor {
            _sys: value
                .dyn_into::<IdbCursorWithValue>()
                .expect("Cursor-returning function did not return an IDBCursorWithValue"),
            _phantom: PhantomData,
        }
    }
}

/// Wrapper for [`IDBCursor`](https://developer.mozilla.org/en-US/docs/Web/API/IDBCursor)
pub struct KeyCursor<Err> {
    // TODO
    _phantom: PhantomData<Err>,
}
