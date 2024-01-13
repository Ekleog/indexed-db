use crate::TransactionBuilder;
use web_sys::{
    js_sys::{Array, JsString},
    IdbDatabase, IdbIndexParameters, IdbObjectStore, IdbObjectStoreParameters,
};

/// Wrapper for [`IDBDatabase`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase)
#[derive(Debug)]
pub struct Database {
    sys: IdbDatabase,
}

impl Database {
    pub(crate) fn from_sys(sys: IdbDatabase) -> Database {
        Database { sys }
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
    pub fn build_object_store<'a>(&self, name: &'a str) -> ObjectStoreBuilder<'a> {
        ObjectStoreBuilder {
            db: self.sys.clone(),
            name,
            options: IdbObjectStoreParameters::new(),
        }
    }

    /// Deletes an [`ObjectStore`]
    ///
    /// Note that this method can only be called from within an `on_upgrade_needed` callback.
    ///
    /// Internally, this uses [`IDBDatabase::deleteObjectStore`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/deleteObjectStore).
    pub fn delete_object_store(&self, name: &str) -> crate::Result<()> {
        self.sys.delete_object_store(name).map_err(|err| match error_name!(&err) {
            Some("InvalidStateError") => crate::Error::InvalidCall,
            Some("TransactionInactiveError") => panic!("Tried to create an object store with the `versionchange` transaction having already aborted"),
            Some("NotFoundError") => crate::Error::DoesNotExist,
            _ => crate::Error::from_js_value(err),
        })
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

pub struct ObjectStoreBuilder<'a> {
    db: IdbDatabase,
    name: &'a str,
    options: IdbObjectStoreParameters,
}

impl<'a> ObjectStoreBuilder<'a> {
    /// Create the object store
    ///
    /// Internally, this uses [`IDBDatabase::createObjectStore`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/createObjectStore).
    pub fn create(self) -> crate::Result<ObjectStoreConfigurator> {
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
            .map(ObjectStoreConfigurator::from_sys)
    }

    /// Set the key path for out-of-line keys
    ///
    /// Internally, this [sets this setting](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/createObjectStore#keypath).
    pub fn key_path(mut self, path: &[&str]) -> Self {
        let arg = Array::new();
        for p in path {
            arg.push(&JsString::from(*p));
        }
        self.options.key_path(Some(&arg));
        self
    }

    /// Enable auto-increment for the key
    ///
    /// Internally, this [sets this setting](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/createObjectStore#autoincrement).
    pub fn auto_increment(mut self) -> Self {
        self.options.auto_increment(true);
        self
    }
}

/// Wrapper for [`IDBObjectStore`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore),
/// specialized for configuring object stores
pub struct ObjectStoreConfigurator {
    sys: IdbObjectStore,
}

impl ObjectStoreConfigurator {
    pub(crate) fn from_sys(sys: IdbObjectStore) -> ObjectStoreConfigurator {
        ObjectStoreConfigurator { sys }
    }

    /// Build an index over this object store
    ///
    /// Note that this method can only be called from within an `on_upgrade_needed` callback. It returns
    /// a builder, and calling the `create` method on this builder will perform the actual creation.
    ///
    /// Internally, this uses [`IDBObjectStore::createIndex`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/createIndex).
    pub fn build_index<'a>(&self, name: &'a str, key_path: &[&str]) -> IndexBuilder<'a> {
        let key = Array::new();
        for p in key_path {
            key.push(&JsString::from(*p));
        }
        IndexBuilder {
            store: self.sys.clone(),
            name,
            key_path: key,
            options: IdbIndexParameters::new(),
        }
    }
}

pub struct IndexBuilder<'a> {
    store: IdbObjectStore,
    name: &'a str,
    key_path: Array,
    options: IdbIndexParameters,
}

impl<'a> IndexBuilder<'a> {
    /// Create the index
    ///
    /// Internally, this uses [`IDBObjectStore::createIndex`](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/createIndex).
    pub fn create(self) -> crate::Result<()> {
        self.store
            .create_index_with_str_sequence_and_optional_parameters(
                self.name,
                &self.key_path,
                &self.options,
            )
            .map_err(|err| match error_name!(&err) {
                Some("ConstraintError") => crate::Error::AlreadyExists,
                Some("InvalidAccessError") => crate::Error::InvalidArgument,
                Some("InvalidStateError") => crate::Error::ObjectStoreWasRemoved,
                Some("SyntaxError") => crate::Error::InvalidKey,
                _ => crate::Error::from_js_value(err),
            })
            .map(|_| ())
    }

    /// Mark this index as unique
    ///
    /// Internally, this sets [this property](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/createIndex#unique).
    pub fn unique(mut self) -> Self {
        self.options.unique(true);
        self
    }

    /// Mark this index as multi-entry
    ///
    /// Internally, this sets [this property](https://developer.mozilla.org/en-US/docs/Web/API/IDBObjectStore/createIndex#multientry).
    pub fn multi_entry(mut self) -> Self {
        self.options.multi_entry(true);
        self
    }
}
