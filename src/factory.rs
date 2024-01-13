use crate::{utils::generic_request, Database};
use web_sys::{
    js_sys::Function,
    wasm_bindgen::{closure::Closure, JsCast, JsValue},
    IdbDatabase, IdbFactory, IdbOpenDbRequest, IdbVersionChangeEvent,
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
    pub fn get() -> crate::Result<Factory> {
        let window = web_sys::window().ok_or(crate::Error::NotInBrowser)?;
        let sys = window
            .indexed_db()
            .map_err(|_| crate::Error::IndexedDbDisabled)?
            .ok_or(crate::Error::IndexedDbDisabled)?;
        Ok(Factory { sys })
    }

    /// Compare two keys for ordering
    ///
    /// Returns an error if one of the two values would not be a valid IndexedDb key.
    ///
    /// This internally uses [`IDBFactory::cmp`](https://developer.mozilla.org/en-US/docs/Web/API/IDBFactory/cmp).
    pub fn cmp(&self, lhs: &JsValue, rhs: &JsValue) -> crate::Result<std::cmp::Ordering> {
        use std::cmp::Ordering::*;
        self.sys
            .cmp(lhs, rhs)
            .map(|v| match v {
                -1 => Less,
                0 => Equal,
                1 => Greater,
                v => panic!("Unexpected result of IDBFactory::cmp: {v}"),
            })
            .map_err(
                |e| match crate::error::name(&e).as_ref().map(|s| s as &str) {
                    Some("DataError") => crate::Error::InvalidKey,
                    _ => crate::Error::from_js_value(e),
                },
            )
    }

    // TODO: add `databases` once web-sys has it

    /// Delete a database
    ///
    /// Returns an error if something failed during the deletion. Note that trying to delete
    /// a database that does not exist will result in a successful result.
    ///
    /// This internally uses [`IDBFactory::deleteDatabase`](https://developer.mozilla.org/en-US/docs/Web/API/IDBFactory/deleteDatabase)
    pub async fn delete_database(&self, name: &str) -> crate::Result<()> {
        generic_request(
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
    /// Note that `version` must be at least `1`. `upgrader` will be called when `version` is higher than the previous
    /// database version, or upon database creation.
    ///
    /// This internally uses [`IDBFactory::open`](https://developer.mozilla.org/en-US/docs/Web/API/IDBFactory/open)
    /// as well as the methods from [`IDBOpenDBRequest`](https://developer.mozilla.org/en-US/docs/Web/API/IDBOpenDBRequest)
    pub async fn open(
        &self,
        name: &str,
        version: u32,
        // TODO: this _could_ not take 'static, but that'd require an unsafe lifetime expansion in order to pass through
        // the `Closure::once` bound. So, let's not do it until a reasonable use case arises.
        upgrader: impl 'static + FnOnce(VersionChangeEvent) -> crate::Result<()>,
    ) -> crate::Result<Database> {
        if version == 0 {
            return Err(crate::Error::VersionMustNotBeZero);
        }

        let open_req = self
            .sys
            .open_with_u32(name, version)
            .map_err(crate::Error::from_js_value)?;

        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let closure = Closure::once(|evt: IdbVersionChangeEvent| {
            let evt = VersionChangeEvent::from_sys(evt);
            match upgrader(evt) {
                Ok(()) => (),
                Err(e) => {
                    tx.send(e).expect(
                        "IDBOpenDBRequest's on_success handler was called before on_upgrade_needed",
                    );
                    return;
                }
            }
        });
        open_req.set_onupgradeneeded(Some(closure.as_ref().dyn_ref::<Function>().unwrap()));

        generic_request(open_req.clone().into())
            .await
            .map_err(crate::Error::from_js_event)?;

        match rx.try_recv() {
            Err(_) => (),
            Ok(err) => return Err(err),
        }

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
pub struct VersionChangeEvent {
    sys: IdbVersionChangeEvent,
    db: Database,
}

impl VersionChangeEvent {
    fn from_sys(sys: IdbVersionChangeEvent) -> VersionChangeEvent {
        let db_sys = sys
            .target()
            .expect("IDBVersionChangeEvent had no target")
            .dyn_into::<IdbOpenDbRequest>()
            .expect("IDBVersionChangeEvent target was not an IDBOpenDBRequest")
            .result()
            .expect("IDBOpenDBRequest had no result in its on_upgrade_needed handler")
            .dyn_into::<IdbDatabase>()
            .expect("IDBOpenDBRequest result was not an IDBDatabase");
        let db = Database::from_sys(db_sys);
        VersionChangeEvent { sys, db }
    }

    /// The version before the database upgrade, clamped to `u32::MAX`
    ///
    /// Internally, this uses [`IDBVersionChangeEvent::old_version`](https://developer.mozilla.org/en-US/docs/Web/API/IDBVersionChangeEvent/oldVersion)
    pub fn old_version(&self) -> u32 {
        self.sys.old_version() as u32
    }

    /// The version after the database upgrade, clamped to `u32::MAX`
    ///
    /// Internally, this uses [`IDBVersionChangeEvent::new_version`](https://developer.mozilla.org/en-US/docs/Web/API/IDBVersionChangeEvent/newVersion)
    pub fn new_version(&self) -> u32 {
        self.sys
            .new_version()
            .expect("IDBVersionChangeEvent did not provide a new version") as u32
    }

    /// The database under creation
    pub fn database(&self) -> &Database {
        &self.db
    }
}
