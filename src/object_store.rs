use std::marker::PhantomData;
use web_sys::IdbObjectStore;

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
    // TODO
}
