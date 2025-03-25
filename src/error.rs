use crate::utils::err_from_event;
use web_sys::{
    wasm_bindgen::{JsCast, JsValue},
    DomException,
};

/// Type alias for convenience
pub type Result<T> = std::result::Result<T, Error>;

/// Error type for all errors from this crate
///
/// The `E` generic argument is used for user-defined error types, eg. when
/// the user provides a callback.
#[derive(Clone, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
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

    /// Cursor finished its range
    #[error("Cursor finished its range")]
    CursorCompleted,
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
        Error::from_dom_exception(err_from_event(evt))
    }
}

pub(crate) fn name(v: &JsValue) -> Option<String> {
    v.dyn_ref::<web_sys::DomException>().map(|v| v.name())
}

// // Approach 1: Allow automatic conversion of `Error<Infallible>` into `Error<Err>`
// //
// // However the conversion clashes with `impl<T> From<T> for T` (probably a limitation
// // in Rust that doesn't support specialization).

// impl<Err> From<Error<std::convert::Infallible>> for Error<Err> {
//     fn from(value: Error<std::convert::Infallible>) -> Self {
//         match value {
//             Error::NotInBrowser => Error::NotInBrowser,
//             Error::IndexedDbDisabled => Error::IndexedDbDisabled,
//             Error::OperationNotSupported => Error::OperationNotSupported,
//             Error::OperationNotAllowed => Error::OperationNotAllowed,
//             Error::InvalidKey => Error::InvalidKey,
//             Error::VersionMustNotBeZero => Error::VersionMustNotBeZero,
//             Error::VersionTooOld => Error::VersionTooOld,
//             Error::InvalidCall => Error::InvalidCall,
//             Error::InvalidArgument => Error::InvalidArgument,
//             Error::AlreadyExists => Error::AlreadyExists,
//             Error::DoesNotExist => Error::DoesNotExist,
//             Error::DatabaseIsClosed => Error::DatabaseIsClosed,
//             Error::ObjectStoreWasRemoved => Error::ObjectStoreWasRemoved,
//             Error::ReadOnly => Error::ReadOnly,
//             Error::FailedClone => Error::FailedClone,
//             Error::InvalidRange => Error::InvalidRange,
//             Error::CursorCompleted => Error::CursorCompleted,
//             Error::User(_) => unreachable!(),
//         }
//     }
// }

// // Approach 2: Introduce a new `IndexdedDbOnlyError` error type when the error never
// // contains a user-defined error type.
// //
// // However the conversion `impl<Err> From<IndexdedDbOnlyError> for Error<Err>` clashes
// // with `Error::User(#[from] E)` :'(

// #[derive(Clone, Debug, thiserror::Error)]
// #[non_exhaustive]
// pub enum IndexdedDbOnlyError {
//     /// Not running in a browser window
//     #[error("Not running in a browser window")]
//     NotInBrowser,

//     /// IndexedDB is disabled
//     #[error("IndexedDB is disabled")]
//     IndexedDbDisabled,

//     /// Operation is not supported by the browser
//     #[error("Operation is not supported by the browser")]
//     OperationNotSupported,

//     /// Operation is not allowed by the user agent
//     #[error("Operation is not allowed by the user agent")]
//     OperationNotAllowed,

//     /// Provided key is not valid
//     #[error("Provided key is not valid")]
//     InvalidKey,

//     /// Version must not be zero
//     #[error("Version must not be zero")]
//     VersionMustNotBeZero,

//     /// Requested version is older than existing version
//     #[error("Requested version is older than existing version")]
//     VersionTooOld,

//     /// The requested function cannot be called from this context
//     #[error("The requested function cannot be called from this context")]
//     InvalidCall,

//     /// The provided arguments are invalid
//     #[error("The provided arguments are invalid")]
//     InvalidArgument,

//     /// Cannot create something that already exists
//     #[error("Cannot create something that already exists")]
//     AlreadyExists,

//     /// Cannot change something that does not exists
//     #[error("Cannot change something that does not exists")]
//     DoesNotExist,

//     /// Database is closed
//     #[error("Database is closed")]
//     DatabaseIsClosed,

//     /// Object store was removed
//     #[error("Object store was removed")]
//     ObjectStoreWasRemoved,

//     /// Transaction is read-only
//     #[error("Transaction is read-only")]
//     ReadOnly,

//     /// Unable to clone
//     #[error("Unable to clone")]
//     FailedClone,

//     /// Invalid range
//     #[error("Invalid range")]
//     InvalidRange,

//     /// Cursor finished its range
//     #[error("Cursor finished its range")]
//     CursorCompleted,
// }

// impl IndexdedDbOnlyError {
//     pub(crate) fn from_dom_exception(err: DomException) -> IndexdedDbOnlyError {
//         match &err.name() as &str {
//             "NotSupportedError" => Self::OperationNotSupported,
//             "NotAllowedError" => Self::OperationNotAllowed,
//             "VersionError" => Self::VersionTooOld,
//             _ => panic!("Unexpected error: {err:?}"),
//         }
//     }

//     pub(crate) fn from_js_value(v: JsValue) -> IndexdedDbOnlyError {
//         let err = v
//             .dyn_into::<web_sys::DomException>()
//             .expect("Trying to parse indexed_db::Error from value that is not a DomException");
//         IndexdedDbOnlyError::from_dom_exception(err)
//     }

//     pub(crate) fn from_js_event(evt: web_sys::Event) -> IndexdedDbOnlyError {
//         IndexdedDbOnlyError::from_dom_exception(err_from_event(evt))
//     }
// }

// impl<Err> From<IndexdedDbOnlyError> for Error<Err> {
//     fn from(value: IndexdedDbOnlyError) -> Self {
//         match value {
//             IndexdedDbOnlyError::NotInBrowser => Error::NotInBrowser,
//             IndexdedDbOnlyError::IndexedDbDisabled => Error::IndexedDbDisabled,
//             IndexdedDbOnlyError::OperationNotSupported => Error::OperationNotSupported,
//             IndexdedDbOnlyError::OperationNotAllowed => Error::OperationNotAllowed,
//             IndexdedDbOnlyError::InvalidKey => Error::InvalidKey,
//             IndexdedDbOnlyError::VersionMustNotBeZero => Error::VersionMustNotBeZero,
//             IndexdedDbOnlyError::VersionTooOld => Error::VersionTooOld,
//             IndexdedDbOnlyError::InvalidCall => Error::InvalidCall,
//             IndexdedDbOnlyError::InvalidArgument => Error::InvalidArgument,
//             IndexdedDbOnlyError::AlreadyExists => Error::AlreadyExists,
//             IndexdedDbOnlyError::DoesNotExist => Error::DoesNotExist,
//             IndexdedDbOnlyError::DatabaseIsClosed => Error::DatabaseIsClosed,
//             IndexdedDbOnlyError::ObjectStoreWasRemoved => Error::ObjectStoreWasRemoved,
//             IndexdedDbOnlyError::ReadOnly => Error::ReadOnly,
//             IndexdedDbOnlyError::FailedClone => Error::FailedClone,
//             IndexdedDbOnlyError::InvalidRange => Error::InvalidRange,
//             IndexdedDbOnlyError::CursorCompleted => Error::CursorCompleted,
//         }
//     }
// }
