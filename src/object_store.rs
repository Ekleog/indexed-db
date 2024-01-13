use std::marker::PhantomData;
use web_sys::IdbObjectStore;

/// Wrapper for [`IDBObjectStore`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore),
/// for use in transactions
#[derive(Debug)]
pub struct ObjectStore<'a, Err> {
    sys: IdbObjectStore,
    _phantom: PhantomData<&'a mut Err>,
}

impl<'a, Err> ObjectStore<'a, Err> {
    pub(crate) fn from_sys(sys: IdbObjectStore) -> ObjectStore<'a, Err> {
        ObjectStore {
            sys,
            _phantom: PhantomData,
        }
    }
    // TODO
}
