use web_sys::wasm_bindgen::{JsCast, JsValue};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Not running in a browser window")]
    NotInBrowser,

    #[error("IndexedDB is disabled")]
    IndexedDbDisabled,

    #[error("Provided key is not valid for IndexedDB")]
    InvalidKey,
}

pub(crate) fn name(v: &JsValue) -> Option<String> {
    v.dyn_ref::<web_sys::DomException>().map(|v| v.name())
}
