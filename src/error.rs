use web_sys::wasm_bindgen::{JsCast, JsValue};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Not running in a browser window")]
    NotInBrowser,

    #[error("IndexedDB is disabled")]
    IndexedDbDisabled,

    #[error("Operation is not supported by the browser")]
    OperationNotSupported,

    #[error("Operation is not allowed by the user agent")]
    OperationNotAllowed,

    #[error("Provided key is not valid for IndexedDB")]
    InvalidKey,
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
