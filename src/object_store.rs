use web_sys::IdbObjectStore;

pub struct ObjectStore {
    sys: IdbObjectStore,
}

impl ObjectStore {
    pub(crate) fn from_sys(sys: IdbObjectStore) -> ObjectStore {
        ObjectStore { sys }
    }
}
