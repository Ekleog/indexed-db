use crate::ObjectStore;
use web_sys::{
    js_sys::{Array, JsString},
    IdbDatabase, IdbObjectStoreParameters,
};

#[derive(Debug)]
pub struct Database {
    sys: IdbDatabase,
}

impl Database {
    pub(crate) fn from_sys(sys: IdbDatabase) -> Database {
        Database { sys }
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
    pub fn create(self) -> crate::Result<ObjectStore> {
        self.db
            .create_object_store_with_optional_parameters(self.name, &self.options)
            .map_err(
                |err| match crate::error::name(&err).as_ref().map(|s| s as &str) {
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
