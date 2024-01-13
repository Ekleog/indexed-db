use crate::{utils::generic_request, Database};
use web_sys::wasm_bindgen::JsValue;

/// Wrapper for [`IDBFactory`](https://developer.mozilla.org/en-US/docs/Web/API/IDBFactory)
pub struct Factory {
    sys: web_sys::IdbFactory,
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
        .map_err(crate::Error::from_js_value)
    }

    /// Open a database
    ///
    /// Returns an error if something failed while opening or upgrading the database.
    /// Blocks until it can actually open the database.
    ///
    /// This internally uses [`IDBFactory::open`](https://developer.mozilla.org/en-US/docs/Web/API/IDBFactory/open)
    /// as well as the methods from [`IDBOpenDBRequest`](https://developer.mozilla.org/en-US/docs/Web/API/IDBOpenDBRequest)
    pub async fn open<E>(
        &self,
        _name: &str,
        _version: u32,
        _upgrader: impl FnOnce(VersionChangeEvent) -> Result<(), crate::Error<E>>,
    ) -> Result<Database, crate::Error<E>> {
        todo!()
    }
}

pub struct VersionChangeEvent {
    // TODO
}
