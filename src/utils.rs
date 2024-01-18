use futures_channel::oneshot;
use futures_util::future::{self, Either};
use std::ops::{Bound, RangeBounds};
use web_sys::{
    js_sys::{Array, Function, JsString, Number, TypeError},
    wasm_bindgen::{closure::Closure, JsCast, JsValue},
    IdbKeyRange, IdbRequest,
};

pub(crate) async fn generic_request(req: IdbRequest) -> Result<web_sys::Event, web_sys::Event> {
    let (success_tx, success_rx) = oneshot::channel();
    let (error_tx, error_rx) = oneshot::channel();

    let on_success = Closure::once(move |v| success_tx.send(v));
    let on_error = Closure::once(move |v| error_tx.send(v));

    req.set_onsuccess(Some(on_success.as_ref().dyn_ref::<Function>().unwrap()));
    req.set_onerror(Some(on_error.as_ref().dyn_ref::<Function>().unwrap()));

    match future::select(success_rx, error_rx).await {
        Either::Left((res, _)) => Ok(res.unwrap()),
        Either::Right((res, _)) => Err(res.unwrap()),
    }
}

pub(crate) fn none_if_undefined(v: JsValue) -> Option<JsValue> {
    if v.is_undefined() {
        None
    } else {
        Some(v)
    }
}

pub(crate) fn array_to_vec(v: JsValue) -> Vec<JsValue> {
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

pub(crate) fn str_slice_to_array(s: &[&str]) -> Array {
    let res = Array::new_with_length(u32::try_from(s.len()).unwrap());
    for (i, v) in s.iter().enumerate() {
        res.set(u32::try_from(i).unwrap(), JsString::from(*v).into());
    }
    res
}

pub(crate) fn map_add_err<Err>(err: JsValue) -> crate::Error<Err> {
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
}

pub(crate) fn map_count_res(res: JsValue) -> usize {
    let num = res
        .dyn_into::<Number>()
        .expect("IDBObjectStore::count did not return a Number");
    assert!(
        Number::is_integer(&num),
        "Number of elements in object store is not an integer"
    );
    num.value_of() as usize
}

pub(crate) fn map_count_err<Err>(err: JsValue) -> crate::Error<Err> {
    match error_name!(&err) {
        Some("InvalidStateError") => crate::Error::ObjectStoreWasRemoved,
        Some("TransactionInactiveError") => {
            panic!("Tried counting in an ObjectStore while the transaction was inactive")
        }
        Some("DataError") => crate::Error::InvalidKey,
        _ => crate::Error::from_js_value(err),
    }
}

pub(crate) fn map_delete_err<Err>(err: JsValue) -> crate::Error<Err> {
    match error_name!(&err) {
        Some("ReadOnlyError") => crate::Error::ReadOnly,
        Some("InvalidStateError") => crate::Error::ObjectStoreWasRemoved,
        Some("TransactionInactiveError") => {
            panic!("Tried deleting from an ObjectStore while the transaction was inactive")
        }
        Some("DataError") => crate::Error::InvalidKey,
        _ => crate::Error::from_js_value(err),
    }
}

pub(crate) fn map_get_err<Err>(err: JsValue) -> crate::Error<Err> {
    match error_name!(&err) {
        Some("InvalidStateError") => crate::Error::ObjectStoreWasRemoved,
        Some("TransactionInactiveError") => {
            panic!("Tried getting from an ObjectStore while the transaction was inactive")
        }
        Some("DataError") => crate::Error::InvalidKey,
        _ => crate::Error::from_js_value(err),
    }
}

pub(crate) fn map_open_cursor_err<Err>(err: JsValue) -> crate::Error<Err> {
    match error_name!(&err) {
        Some("InvalidStateError") => crate::Error::ObjectStoreWasRemoved,
        Some("TransactionInactiveError") => {
            panic!("Tried opening a Cursor on an ObjectStore while the transaction was inactive")
        }
        Some("DataError") => crate::Error::InvalidKey,
        _ => crate::Error::from_js_value(err),
    }
}

pub(crate) fn map_cursor_advance_err<Err>(err: JsValue) -> crate::Error<Err> {
    match error_name!(&err) {
        Some("InvalidStateError") => crate::Error::CursorCompleted,
        Some("TransactionInactiveError") => {
            panic!("Tried advancing a Cursor on an ObjectStore while the transaction was inactive")
        }
        None if err.has_type::<TypeError>() => crate::Error::InvalidArgument,
        _ => crate::Error::from_js_value(err),
    }
}

pub(crate) fn map_cursor_advance_until_err<Err>(err: JsValue) -> crate::Error<Err> {
    match error_name!(&err) {
        Some("InvalidStateError") => crate::Error::CursorCompleted,
        Some("TransactionInactiveError") => {
            panic!("Tried advancing a Cursor on an ObjectStore while the transaction was inactive")
        }
        Some("DataError") => crate::Error::InvalidKey,
        _ => crate::Error::from_js_value(err),
    }
}

pub(crate) fn map_cursor_advance_until_primary_key_err<Err>(err: JsValue) -> crate::Error<Err> {
    match error_name!(&err) {
        Some("InvalidStateError") => crate::Error::CursorCompleted,
        Some("TransactionInactiveError") => {
            panic!("Tried advancing a Cursor on an ObjectStore while the transaction was inactive")
        }
        Some("DataError") => crate::Error::InvalidKey,
        Some("InvalidAccessError") => crate::Error::InvalidArgument,
        _ => crate::Error::from_js_value(err),
    }
}

pub(crate) fn map_cursor_delete_err<Err>(err: JsValue) -> crate::Error<Err> {
    match error_name!(&err) {
        Some("InvalidStateError") => crate::Error::CursorCompleted,
        Some("TransactionInactiveError") => {
            panic!("Tried advancing a Cursor on an ObjectStore while the transaction was inactive")
        }
        Some("ReadOnlyError") => crate::Error::ReadOnly,
        _ => crate::Error::from_js_value(err),
    }
}

pub(crate) fn map_cursor_update_err<Err>(err: JsValue) -> crate::Error<Err> {
    match error_name!(&err) {
        Some("InvalidStateError") => crate::Error::CursorCompleted,
        Some("TransactionInactiveError") => {
            panic!("Tried advancing a Cursor on an ObjectStore while the transaction was inactive")
        }
        Some("ReadOnlyError") => crate::Error::ReadOnly,
        Some("DataError") => crate::Error::InvalidKey,
        Some("DataCloneError") => crate::Error::FailedClone,
        _ => crate::Error::from_js_value(err),
    }
}

pub(crate) fn make_key_range<Err>(range: impl RangeBounds<JsValue>) -> crate::Result<JsValue, Err> {
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
    .map_err(|err| match error_name!(&err) {
        Some("DataError") => crate::Error::InvalidKey,
        _ => crate::Error::from_js_value(err),
    })
}
