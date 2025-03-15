use crate::{transaction::unsafe_jar, utils::generic_request, Database, Transaction};
use futures_channel::oneshot;
use futures_util::{
    future::{self, Either},
    pin_mut, FutureExt,
};
use std::marker::PhantomData;
use web_sys::{
    js_sys::{self, Function},
    wasm_bindgen::{closure::Closure, JsCast, JsValue},
    IdbDatabase, IdbFactory, IdbOpenDbRequest, IdbVersionChangeEvent, WorkerGlobalScope,
};

/// Wrapper for [`IDBFactory`](https://developer.mozilla.org/en-US/docs/Web/API/IDBFactory)
///
/// Note that it is quite likely that type inference will fail on the `Err` generic argument here.
/// This argument is the type of user-defined errors that will be passed through transactions and
/// callbacks.
/// You should set it to whatever error type your program uses around the `indexed-db`-using code.
#[derive(Debug)]
pub struct Factory<Err> {
    sys: IdbFactory,
    _phantom: PhantomData<Err>,
}

impl<Err: 'static> Factory<Err> {
    /// Retrieve the global `Factory` from the browser
    ///
    /// This internally uses [`indexedDB`](https://developer.mozilla.org/en-US/docs/Web/API/indexedDB).
    pub fn get() -> crate::Result<Factory<Err>, Err> {
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

        Ok(Factory {
            sys,
            _phantom: PhantomData,
        })
    }

    /// Compare two keys for ordering
    ///
    /// Returns an error if one of the two values would not be a valid IndexedDb key.
    ///
    /// This internally uses [`IDBFactory::cmp`](https://developer.mozilla.org/en-US/docs/Web/API/IDBFactory/cmp).
    pub fn cmp(&self, lhs: &JsValue, rhs: &JsValue) -> crate::Result<std::cmp::Ordering, Err> {
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
    pub async fn delete_database(&self, name: &str) -> crate::Result<(), Err> {
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
    /// Note that `version` must be at least `1`. `on_upgrade_needed` will be called when `version` is higher
    /// than the previous database version, or upon database creation.
    ///
    /// This internally uses [`IDBFactory::open`](https://developer.mozilla.org/en-US/docs/Web/API/IDBFactory/open)
    /// as well as the methods from [`IDBOpenDBRequest`](https://developer.mozilla.org/en-US/docs/Web/API/IDBOpenDBRequest)
    pub async fn open(
        &self,
        name: &str,
        version: u32,
        on_upgrade_needed: impl 'static + AsyncFnOnce(VersionChangeEvent<Err>) -> crate::Result<(), Err>,
    ) -> crate::Result<Database<Err>, Err> {
        if version == 0 {
            return Err(crate::Error::VersionMustNotBeZero);
        }

        let open_req = self
            .sys
            .open_with_u32(name, version)
            .map_err(crate::Error::from_js_value)?;

        let (upgrade_tx, upgrade_rx) = oneshot::channel();
        let on_upgrade_needed = Closure::once(move |evt: IdbVersionChangeEvent| {
            let evt = VersionChangeEvent::from_sys(evt);
            let transaction = evt.transaction().as_sys().clone();
            let fut = {
                let transaction = transaction.clone();
                async move {
                    let res = on_upgrade_needed(evt).await;
                    let return_value = match &res {
                        Ok(_) => Ok(()),
                        Err(_) => Err(()),
                    };
                    if let Err(_) = upgrade_tx.send(res) {
                        // Opening request was cancelled by dropping, abort the transaction
                        let _ = transaction.abort();
                    }
                    return_value
                }
            };
            unsafe_jar::run(transaction, fut);
        });
        open_req.set_onupgradeneeded(Some(
            on_upgrade_needed.as_ref().dyn_ref::<Function>().unwrap(),
        ));

        let completion_fut = generic_request(open_req.clone().into());
        pin_mut!(completion_fut);

        let res = future::select(upgrade_rx, completion_fut).await;
        if unsafe_jar::POLLED_FORBIDDEN_THING.get() {
            panic!("Transaction blocked without any request under way");
        }

        match res {
            Either::Right((completion, _)) => {
                completion.map_err(crate::Error::from_js_event)?;
            }
            Either::Left((upgrade_res, completion_fut)) => {
                let upgrade_res = upgrade_res.expect("Closure dropped before its end of scope");
                upgrade_res?;
                completion_fut.await.map_err(crate::Error::from_js_event)?;
            }
        }

        let db = open_req
            .result()
            .map_err(crate::Error::from_js_value)?
            .dyn_into::<IdbDatabase>()
            .expect("Result of successful IDBOpenDBRequest is not an IDBDatabase");

        Ok(Database::from_sys(db))
    }

    /// Open a database at the latest version
    ///
    /// Returns an error if something failed while opening.
    /// Blocks until it can actually open the database.
    ///
    /// This internally uses [`IDBFactory::open`](https://developer.mozilla.org/en-US/docs/Web/API/IDBFactory/open)
    /// as well as the methods from [`IDBOpenDBRequest`](https://developer.mozilla.org/en-US/docs/Web/API/IDBOpenDBRequest)
    pub async fn open_latest_version(&self, name: &str) -> crate::Result<Database<Err>, Err> {
        let open_req = self.sys.open(name).map_err(crate::Error::from_js_value)?;

        let completion_fut = generic_request(open_req.clone().into())
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
    db: Database<Err>,
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
    pub fn database(&self) -> &Database<Err> {
        &self.db
    }

    /// The `versionchange` transaction that triggered this event
    ///
    /// This transaction can be used to submit further requests.
    pub fn transaction(&self) -> &Transaction<Err> {
        &self.transaction
    }
}
