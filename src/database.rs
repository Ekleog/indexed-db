use web_sys::IdbDatabase;

#[derive(Debug)]
pub struct Database {
    sys: IdbDatabase,
}

impl Database {
    pub(crate) fn from_sys(sys: IdbDatabase) -> Database {
        Database { sys }
    }
}
