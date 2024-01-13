use web_sys::{
    wasm_bindgen::{JsCast, JsValue},
    DomException,
};

/// Type alias for convenience
pub type Result<T> = std::result::Result<T, Error>;

// TODO: replace with ! once Rust 2024 lands
// At this point we'll probably also be able to remove the `: std::error::Error` bounds everywhere,
// and hopefully the `impl From for Error` too
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

    /// Version must not be zero
    #[error("Version must not be zero")]
    VersionMustNotBeZero,

    /// Requested version is older than existing version
    #[error("Requested version is older than existing version")]
    VersionTooOld,

    /// User-provided error to pass through `indexed-db` code
    #[error(transparent)]
    User(#[from] E),
}

impl<E: std::error::Error> From<Error<Void>> for Error<E> {
    fn from(o: Error<Void>) -> Error<E> {
        match o {
            Error::User(e) => match e {},
            Error::NotInBrowser => Error::NotInBrowser,
            Error::IndexedDbDisabled => Error::IndexedDbDisabled,
            Error::OperationNotSupported => Error::OperationNotSupported,
            Error::OperationNotAllowed => Error::OperationNotAllowed,
            Error::InvalidKey => Error::InvalidKey,
            Error::VersionMustNotBeZero => Error::VersionMustNotBeZero,
            Error::VersionTooOld => Error::VersionTooOld,
        }
    }
}

impl Error {
    pub(crate) fn from_dom_exception(err: DomException) -> Error {
        match &err.name() as &str {
            "NotSupportedError" => crate::Error::OperationNotSupported,
            "NotAllowedError" => crate::Error::OperationNotAllowed,
            "VersionError" => crate::Error::VersionTooOld,
            _ => panic!("Unexpected error: {err:?}"),
        }
    }

    pub(crate) fn from_js_value(v: JsValue) -> Error {
        let err = v
            .dyn_into::<web_sys::DomException>()
            .expect("Trying to parse indexed_db::Error from value that is not a DomException");
        Error::from_dom_exception(err)
    }

    pub(crate) fn from_js_event(evt: web_sys::Event) -> Error {
        let idb_request = evt
            .target()
            .expect("Trying to parse indexed_db::Error from an event that has no target")
            .dyn_into::<web_sys::IdbRequest>()
            .expect(
                "Trying to parse indexed_db::Error from an event that is not from an IDBRequest",
            );
        Error::from_dom_exception(
            idb_request
                .error()
                .expect("Failed to retrieve the error from the IDBRequest that called on_error")
                .expect("IDBRequest::error did not return a DOMException"),
        )
    }
}

pub(crate) fn name(v: &JsValue) -> Option<String> {
    v.dyn_ref::<web_sys::DomException>().map(|v| v.name())
}
