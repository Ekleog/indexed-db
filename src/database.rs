use crate::transaction::TransactionBuilder;
use web_sys::IdbDatabase;

/// Wrapper for [`IDBDatabase`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase)
///
/// Note that dropping this wrapper automatically calls [`IDBDatabase::close`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/close)
/// to request the underlying database connection to be closed (the actual database close
/// occuring asynchronously with no way for the client to identify when this happens).
#[derive(Debug)]
pub struct OwnedDatabase(
    /// This field only switches to `None` to prevent database close on drop when
    /// `OwnedDatabase::into_manual_close` is used.
    pub(crate) Option<Database>,
);

impl OwnedDatabase {
    /// Convert this into a [`Database`] that does not automatically close the connection when dropped.
    ///
    /// The resulting [`Database`] is `Clone` without requiring reference-counting, which can be more convenient than refcounting [`OwnedDatabase`]
    pub fn into_manual_close(mut self) -> Database {
        self.0.take().expect("Database already taken")
    }

    /// Explicitly closes this database connection
    ///
    /// Calling this method is strictly equivalent to dropping the [`OwnedDatabase`] instance.
    /// This method is only provided for symmetry with [`Database::close`].
    pub fn close(self) {
        // `self` is dropped here
    }
}

impl std::ops::Deref for OwnedDatabase {
    type Target = Database;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("Database already taken")
    }
}

impl Drop for OwnedDatabase {
    fn drop(&mut self) {
        match self.0.take() {
            Some(db) => db.close(),
            None => {} // Database was taken with `into_manual_close`
        }
    }
}

/// Wrapper for [`IDBDatabase`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase)
///
/// Unlike[``OwnedDatabase`], this does not automatically close the database connection when dropped.
///
/// Note that failing to close the database connection prior to dropping this wrapper will let the connection
/// remain open until the Javascript garbage collector kicks in, which typically can take tens of seconds
/// during which any new attempt to e.g. delete or open with upgrade the database will hang.
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
