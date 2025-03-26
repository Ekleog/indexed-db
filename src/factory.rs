use crate::{
    database::OwnedDatabase,
    transaction::unsafe_jar,
    utils::{non_transaction_request, str_slice_to_array},
    Database, ObjectStore, Transaction,
};
use futures_util::{pin_mut, FutureExt};
use std::{
    cell::{Cell, RefCell},
    convert::Infallible,
    marker::PhantomData,
};
use web_sys::{
    js_sys::{self, Function, JsString},
    wasm_bindgen::{closure::Closure, JsCast, JsValue},
    IdbDatabase, IdbFactory, IdbObjectStoreParameters, IdbOpenDbRequest, IdbTransaction,
    IdbVersionChangeEvent, WorkerGlobalScope,
};

/// Wrapper for [`IDBFactory`](https://developer.mozilla.org/en-US/docs/Web/API/IDBFactory)
#[derive(Debug)]
pub struct Factory {
    sys: IdbFactory,
}

impl Factory {
    /// Retrieve the global `Factory` from the browser
    ///
    /// This internally uses [`indexedDB`](https://developer.mozilla.org/en-US/docs/Web/API/indexedDB).
    pub fn get() -> crate::Result<Factory, Infallible> {
        let indexed_db = if let Some(window) = web_sys::window() {
            window.indexed_db()
        } else if let Ok(worker_scope) = js_sys::global().dyn_into::<WorkerGlobalScope>() {
            worker_scope.indexed_db()
        } else {
            return Err(crate::Error::NotInBrowser);
        };

        let sys = indexed_db
            .map_err(|_| crate::Error::IndexedDbDisabled)?
            .ok_or(crate::Error::IndexedDbDisabled)?;

        Ok(Factory { sys })
    }

    /// Compare two keys for ordering
    ///
    /// Returns an error if one of the two values would not be a valid IndexedDb key.
    ///
    /// This internally uses [`IDBFactory::cmp`](https://developer.mozilla.org/en-US/docs/Web/API/IDBFactory/cmp).
    pub fn cmp(
        &self,
        lhs: &JsValue,
        rhs: &JsValue,
    ) -> crate::Result<std::cmp::Ordering, Infallible> {
        use std::cmp::Ordering::*;
        self.sys
            .cmp(lhs, rhs)
            .map(|v| match v {
                -1 => Less,
                0 => Equal,
                1 => Greater,
                v => panic!("Unexpected result of IDBFactory::cmp: {v}"),
            })
            .map_err(|e| match error_name!(&e) {
                Some("DataError") => crate::Error::InvalidKey,
                _ => crate::Error::from_js_value(e),
            })
    }

    // TODO: add `databases` once web-sys has it

    /// Delete a database
    ///
    /// Returns an error if something failed during the deletion. Note that trying to delete
    /// a database that does not exist will result in a successful result.
    ///
    /// This internally uses [`IDBFactory::deleteDatabase`](https://developer.mozilla.org/en-US/docs/Web/API/IDBFactory/deleteDatabase)
    pub async fn delete_database(&self, name: &str) -> crate::Result<(), Infallible> {
        non_transaction_request(
            self.sys
                .delete_database(name)
                .map_err(crate::Error::from_js_value)?
                .into(),
        )
        .await
        .map(|_| ())
        .map_err(crate::Error::from_js_event)
    }

    /// Open a database
    ///
    /// Returns an error if something failed while opening or upgrading the database.
    /// Blocks until it can actually open the database.
    ///
    /// Note that `version` must be at least `1`. `on_upgrade_needed` will be called when `version` is higher
    /// than the previous database version, or upon database creation.
    ///
    /// This internally uses [`IDBFactory::open`](https://developer.mozilla.org/en-US/docs/Web/API/IDBFactory/open)
    /// as well as the methods from [`IDBOpenDBRequest`](https://developer.mozilla.org/en-US/docs/Web/API/IDBOpenDBRequest)
    // TODO: once the try_trait_v2 feature is stabilized, we can finally stop carrying any `Err` generic
    pub async fn open<Err: 'static>(
        &self,
        name: &str,
        version: u32,
        on_upgrade_needed: impl AsyncFnOnce(VersionChangeEvent<Err>) -> crate::Result<(), Err>,
    ) -> crate::Result<OwnedDatabase, Err> {
        if version == 0 {
            return Err(crate::Error::VersionMustNotBeZero);
        }

        let open_req = self
            .sys
            .open_with_u32(name, version)
            .map_err(crate::Error::from_js_value)?;

        let result = RefCell::new(None);
        let result = &result;
        let (finished_tx, finished_rx) = futures_channel::oneshot::channel();
        let ran_upgrade_cb = Cell::new(false);
        let ran_upgrade_cb = &ran_upgrade_cb;

        unsafe_jar::extend_lifetime_to_scope_and_run(
            Box::new(
                move |(transaction, event): (IdbTransaction, VersionChangeEvent<Err>)| {
                    let fut = async move {
                        ran_upgrade_cb.set(true);
                        on_upgrade_needed(event).await
                    };
                    unsafe_jar::RunnableTransaction::new(transaction, fut, result, finished_tx)
                },
            ),
            async move |s| {
                // Separate variable to keep the closure alive until opening completed
                let on_upgrade_needed = Closure::once(move |evt: IdbVersionChangeEvent| {
                    let evt = VersionChangeEvent::from_sys(evt);
                    let transaction = evt.transaction().as_sys().clone();
                    s.run((transaction, evt))
                });
                open_req.set_onupgradeneeded(Some(
                    on_upgrade_needed.as_ref().dyn_ref::<Function>().unwrap(),
                ));

                let completion_res = non_transaction_request(open_req.clone().into()).await;
                if ran_upgrade_cb.get() {
                    // The upgrade callback was run, so we need to wait for its result to reach us
                    let _ = finished_rx.await;
                    let result = result
                        .borrow_mut()
                        .take()
                        .expect("Finished was called without the result being available");
                    match result {
                        unsafe_jar::TransactionResult::PolledForbiddenThing => {
                            panic!("Transaction blocked without any request under way")
                        }
                        unsafe_jar::TransactionResult::Done(upgrade_res) => upgrade_res?,
                    }
                }
                completion_res.map_err(crate::Error::from_js_event)?;

                let db = open_req
                    .result()
                    .map_err(crate::Error::from_js_value)?
                    .dyn_into::<IdbDatabase>()
                    .expect("Result of successful IDBOpenDBRequest is not an IDBDatabase");

                Ok(OwnedDatabase::make_auto_close(Database::from_sys(db)))
            },
        )
        .await
    }

    /// Open a database at the latest version
    ///
    /// Returns an error if something failed while opening.
    /// Blocks until it can actually open the database.
    ///
    /// This internally uses [`IDBFactory::open`](https://developer.mozilla.org/en-US/docs/Web/API/IDBFactory/open)
    /// as well as the methods from [`IDBOpenDBRequest`](https://developer.mozilla.org/en-US/docs/Web/API/IDBOpenDBRequest)
    pub async fn open_latest_version(&self, name: &str) -> crate::Result<Database, Infallible> {
        let open_req = self.sys.open(name).map_err(crate::Error::from_js_value)?;

        let completion_fut = non_transaction_request(open_req.clone().into())
            .map(|res| res.map_err(crate::Error::from_js_event));
        pin_mut!(completion_fut);

        completion_fut.await?;

        let db = open_req
            .result()
            .map_err(crate::Error::from_js_value)?
            .dyn_into::<IdbDatabase>()
            .expect("Result of successful IDBOpenDBRequest is not an IDBDatabase");

        Ok(Database::from_sys(db))
    }
}

/// Wrapper for [`IDBVersionChangeEvent`](https://developer.mozilla.org/en-US/docs/Web/API/IDBVersionChangeEvent)
#[derive(Debug)]
pub struct VersionChangeEvent<Err> {
    sys: IdbVersionChangeEvent,
    db: Database,
    transaction: Transaction<Err>,
}

impl<Err> VersionChangeEvent<Err> {
    fn from_sys(sys: IdbVersionChangeEvent) -> VersionChangeEvent<Err> {
        let db_req = sys
            .target()
            .expect("IDBVersionChangeEvent had no target")
            .dyn_into::<IdbOpenDbRequest>()
            .expect("IDBVersionChangeEvent target was not an IDBOpenDBRequest");
        let db_sys = db_req
            .result()
            .expect("IDBOpenDBRequest had no result in its on_upgrade_needed handler")
            .dyn_into::<IdbDatabase>()
            .expect("IDBOpenDBRequest result was not an IDBDatabase");
        let transaction_sys = db_req
            .transaction()
            .expect("IDBOpenDBRequest had no associated transaction");
        let db = Database::from_sys(db_sys);
        let transaction = Transaction::from_sys(transaction_sys);
        VersionChangeEvent {
            sys,
            db,
            transaction,
        }
    }

    /// The version before the database upgrade, clamped to `u32::MAX`
    ///
    /// Internally, this uses [`IDBVersionChangeEvent::oldVersion`](https://developer.mozilla.org/en-US/docs/Web/API/IDBVersionChangeEvent/oldVersion)
    pub fn old_version(&self) -> u32 {
        self.sys.old_version() as u32
    }

    /// The version after the database upgrade, clamped to `u32::MAX`
    ///
    /// Internally, this uses [`IDBVersionChangeEvent::newVersion`](https://developer.mozilla.org/en-US/docs/Web/API/IDBVersionChangeEvent/newVersion)
    pub fn new_version(&self) -> u32 {
        self.sys
            .new_version()
            .expect("IDBVersionChangeEvent did not provide a new version") as u32
    }

    /// The database under creation
    pub fn database(&self) -> &Database {
        &self.db
    }

    /// Build an [`ObjectStore`]
    ///
    /// This returns a builder, and calling the `create` method on this builder will perform the actual creation.
    ///
    /// Internally, this uses [`IDBDatabase::createObjectStore`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/createObjectStore).
    pub fn build_object_store<'a>(&self, name: &'a str) -> ObjectStoreBuilder<'a, Err> {
        ObjectStoreBuilder {
            db: self.db.as_sys().clone(),
            name,
            options: IdbObjectStoreParameters::new(),
            _phantom: PhantomData,
        }
    }

    /// Deletes an [`ObjectStore`]
    ///
    /// Internally, this uses [`IDBDatabase::deleteObjectStore`](https://developer.mozilla.org/en-US/docs/Web/API/IDBDatabase/deleteObjectStore).
    pub fn delete_object_store(&self, name: &str) -> crate::Result<(), Err> {
        self.db.as_sys().delete_object_store(name).map_err(|err| match error_name!(&err) {
                Some("InvalidStateError") => crate::Error::InvalidCall,
                Some("TransactionInactiveError") => panic!("Tried to delete an object store with the `versionchange` transaction having already aborted"),
                Some("NotFoundError") => crate::Error::DoesNotExist,
                _ => crate::Error::from_js_value(err),
            })
    }

    /// The `versionchange` transaction that triggered this event
    ///
    /// This transaction can be used to submit further requests.
    pub fn transaction(&self) -> &Transaction<Err> {
        &self.transaction
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
