use crate::transaction::TransactionBuilder;
use web_sys::IdbDatabase;

/// Wrapper for [`IDBDatabase`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase)
#[derive(Debug)]
pub struct Database {
    sys: IdbDatabase,
}

impl Database {
    pub(crate) fn from_sys(sys: IdbDatabase) -> Database {
        Database { sys }
    }

    pub(crate) fn as_sys(&self) -> &IdbDatabase {
        &self.sys
    }

    /// The name of this database
    ///
    /// Internally, this uses [`IDBDatabase::name`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/name).
    pub fn name(&self) -> String {
        self.sys.name()
    }

    /// The version of this database, clamped at `u32::MAX`
    ///
    /// Internally, this uses [`IDBDatabase::version`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/version).
    pub fn version(&self) -> u32 {
        self.sys.version() as u32
    }

    /// The names of all [`ObjectStore`]s in this [`Database`]
    ///
    /// Internally, this uses [`IDBDatabase::objectStoreNames`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/objectStoreNames).
    pub fn object_store_names(&self) -> Vec<String> {
        let names = self.sys.object_store_names();
        let len = names.length();
        let mut res = Vec::with_capacity(usize::try_from(len).unwrap());
        for i in 0..len {
            res.push(
                names
                    .get(i)
                    .expect("DOMStringList did not contain as many elements as its length"),
            );
        }
        res
    }

    /// Run a transaction
    ///
    /// This will open the object stores identified by `stores`. See the methods of [`TransactionBuilder`]
    /// for more details about how transactions actually happen.
    pub fn transaction(&self, stores: &[&str]) -> TransactionBuilder {
        TransactionBuilder::from_names(self.sys.clone(), stores)
    }

    /// Closes this database connection
    ///
    /// Note that the closing will actually happen asynchronously with no way for the client to
    /// identify when the database was closed.
    ///
    /// Internally, this uses [`IDBDatabase::close`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/close).
    pub fn close(&self) {
        self.sys.close();
    }
}
