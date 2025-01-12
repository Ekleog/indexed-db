use crate::{transaction::TransactionBuilder, utils::str_slice_to_array, ObjectStore};
use std::marker::PhantomData;
use web_sys::{js_sys::JsString, IdbDatabase, IdbObjectStoreParameters};

/// Wrapper for [`IDBDatabase`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase)
#[derive(Debug)]
pub struct Database<Err> {
    sys: IdbDatabase,
    _phantom: PhantomData<Err>,
}

impl<Err> Database<Err> {
    pub(crate) fn from_sys(sys: IdbDatabase) -> Database<Err> {
        Database {
            sys,
            _phantom: PhantomData,
        }
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

    /// Build an [`ObjectStore`]
    ///
    /// Note that this method can only be called from within an `on_upgrade_needed` callback. It returns
    /// a builder, and calling the `create` method on this builder will perform the actual creation.
    ///
    /// Internally, this uses [`IDBDatabase::createObjectStore`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/createObjectStore).
    pub fn build_object_store<'a>(&self, name: &'a str) -> ObjectStoreBuilder<'a, Err> {
        ObjectStoreBuilder {
            db: self.sys.clone(),
            name,
            options: IdbObjectStoreParameters::new(),
            _phantom: PhantomData,
        }
    }

    /// Deletes an [`ObjectStore`]
    ///
    /// Note that this method can only be called from within an `on_upgrade_needed` callback.
    ///
    /// Internally, this uses [`IDBDatabase::deleteObjectStore`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/deleteObjectStore).
    pub fn delete_object_store(&self, name: &str) -> crate::Result<(), Err> {
        self.sys.delete_object_store(name).map_err(|err| match error_name!(&err) {
            Some("InvalidStateError") => crate::Error::InvalidCall,
            Some("TransactionInactiveError") => panic!("Tried to delete an object store with the `versionchange` transaction having already aborted"),
            Some("NotFoundError") => crate::Error::DoesNotExist,
            _ => crate::Error::from_js_value(err),
        })
    }

    /// Run a transaction
    ///
    /// This will open the object stores identified by `stores`. See the methods of [`TransactionBuilder`]
    /// for more details about how transactions actually happen.
    pub fn transaction(&self, stores: &[&str]) -> TransactionBuilder<Err> {
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

/// Helper to build an object store
pub struct ObjectStoreBuilder<'a, Err> {
    db: IdbDatabase,
    name: &'a str,
    options: IdbObjectStoreParameters,
    _phantom: PhantomData<Err>,
}

impl<'a, Err> ObjectStoreBuilder<'a, Err> {
    /// Create the object store
    ///
    /// Internally, this uses [`IDBDatabase::createObjectStore`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/createObjectStore).
    pub fn create(self) -> crate::Result<ObjectStore<Err>, Err> {
        self.db
            .create_object_store_with_optional_parameters(self.name, &self.options)
            .map_err(
                |err| match error_name!(&err) {
                    Some("InvalidStateError") => crate::Error::InvalidCall,
                    Some("TransactionInactiveError") => panic!("Tried to create an object store with the `versionchange` transaction having already aborted"),
                    Some("ConstraintError") => crate::Error::AlreadyExists,
                    Some("InvalidAccessError") => crate::Error::InvalidArgument,
                    _ => crate::Error::from_js_value(err),
                },
            )
            .map(ObjectStore::from_sys)
    }

    /// Set the key path for out-of-line keys
    ///
    /// If you want to use a compound primary key made of multiple attributes, please see [`ObjectStoreBuilder::compound_key_path`].
    ///
    /// Internally, this [sets this setting](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/createObjectStore#keypath).
    pub fn key_path(self, path: &str) -> Self {
        self.options.set_key_path(&JsString::from(path));
        self
    }

    /// Set the key path for out-of-line keys
    ///
    /// Internally, this [sets this setting](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/createObjectStore#keypath).
    pub fn compound_key_path(self, paths: &[&str]) -> Self {
        self.options.set_key_path(&str_slice_to_array(paths));
        self
    }

    /// Enable auto-increment for the key
    ///
    /// Internally, this [sets this setting](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/createObjectStore#autoincrement).
    pub fn auto_increment(self) -> Self {
        self.options.set_auto_increment(true);
        self
    }
}
