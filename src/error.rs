use web_sys::wasm_bindgen::{JsCast, JsValue};

/// Type alias for convenience
pub type Result<T> = std::result::Result<T, Error>;

// TODO: replace with ! once Rust 2024 lands
#[doc(hidden)]
#[derive(Debug)]
pub enum Void {}

/// Error type for all errors from this crate
///
/// The `E` generic argument is used for when user-defined error types should
/// be allowed, eg. when the user provides a callback.
#[derive(Clone, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error<E = Void> {
    /// Not running in a browser window
    #[error("Not running in a browser window")]
    NotInBrowser,

    /// IndexedDB is disabled
    #[error("IndexedDB is disabled")]
    IndexedDbDisabled,

    /// Operation is not supported by the browser
    #[error("Operation is not supported by the browser")]
    OperationNotSupported,

    /// Operation is not allowed by the user agent
    #[error("Operation is not allowed by the user agent")]
    OperationNotAllowed,

    /// Provided key is not valid for IndexedDB
    #[error("Provided key is not valid for IndexedDB")]
    InvalidKey,

    /// User-provided error to pass through `indexed-db` code
    #[error(transparent)]
    User(#[from] E),
}

impl Error {
    pub(crate) fn from_js_value(v: JsValue) -> Error {
        let err = v
            .dyn_into::<web_sys::DomException>()
            .expect("Trying to parse indexed_db::Error from value that is not a DomException");
        match &err.name() as &str {
            "NotSupportedError" => crate::Error::OperationNotSupported,
            "NotAllowedError" => crate::Error::OperationNotAllowed,
            _ => panic!("Unexpected error: {err:?}"),
        }
    }
}

pub(crate) fn name(v: &JsValue) -> Option<String> {
    v.dyn_ref::<web_sys::DomException>().map(|v| v.name())
}
