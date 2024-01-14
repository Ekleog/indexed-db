use web_sys::{
    wasm_bindgen::{JsCast, JsValue},
    DomException,
};

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

    /// Provided key is not valid
    #[error("Provided key is not valid")]
    InvalidKey,

    /// Version must not be zero
    #[error("Version must not be zero")]
    VersionMustNotBeZero,

    /// Requested version is older than existing version
    #[error("Requested version is older than existing version")]
    VersionTooOld,

    /// The requested function cannot be called from this context
    #[error("The requested function cannot be called from this context")]
    InvalidCall,

    /// The provided arguments are invalid
    #[error("The provided arguments are invalid")]
    InvalidArgument,

    /// Cannot create something that already exists
    #[error("Cannot create something that already exists")]
    AlreadyExists,

    /// Cannot change something that does not exists
    #[error("Cannot change something that does not exists")]
    DoesNotExist,

    /// Database is closed
    #[error("Database is closed")]
    DatabaseIsClosed,

    /// Object store was removed
    #[error("Object store was removed")]
    ObjectStoreWasRemoved,

    /// Transaction is read-only
    #[error("Transaction is read-only")]
    ReadOnly,

    /// Unable to clone
    #[error("Unable to clone")]
    FailedClone,

    /// Invalid range
    #[error("Invalid range")]
    InvalidRange,

    /// User-provided error to pass through `indexed-db` code
    #[error(transparent)]
    User(#[from] E),
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

    pub(crate) fn into_user<E>(self) -> Error<E> {
        match self {
            Error::NotInBrowser => Error::NotInBrowser,
            Error::IndexedDbDisabled => Error::IndexedDbDisabled,
            Error::OperationNotSupported => Error::OperationNotSupported,
            Error::OperationNotAllowed => Error::OperationNotAllowed,
            Error::InvalidKey => Error::InvalidKey,
            Error::VersionMustNotBeZero => Error::VersionMustNotBeZero,
            Error::VersionTooOld => Error::VersionTooOld,
            Error::InvalidCall => Error::InvalidCall,
            Error::InvalidArgument => Error::InvalidArgument,
            Error::AlreadyExists => Error::AlreadyExists,
            Error::DoesNotExist => Error::DoesNotExist,
            Error::DatabaseIsClosed => Error::DatabaseIsClosed,
            Error::ObjectStoreWasRemoved => Error::ObjectStoreWasRemoved,
            Error::ReadOnly => Error::ReadOnly,
            Error::FailedClone => Error::FailedClone,
            Error::InvalidRange => Error::InvalidRange,
            Error::User(u) => match u {},
        }
    }
}

pub(crate) fn name(v: &JsValue) -> Option<String> {
    v.dyn_ref::<web_sys::DomException>().map(|v| v.name())
}
