use web_sys::{
    js_sys::{Array, JsString},
    wasm_bindgen::JsValue,
    IdbTransactionMode,
};

/// Helper to build a transaction
pub struct TransactionBuilder {
    stores: JsValue,
    mode: IdbTransactionMode,
    // TODO: add support for transaction durability when web-sys gets it
}

impl TransactionBuilder {
    pub(crate) fn from_names(names: &[&str]) -> TransactionBuilder {
        let stores = Array::new();
        for s in names {
            stores.push(&JsString::from(*s));
        }
        TransactionBuilder {
            stores: stores.into(),
            mode: IdbTransactionMode::Readonly,
        }
    }

    /// Allow writes in this transaction
    pub fn rw(mut self) -> Self {
        self.mode = IdbTransactionMode::Readwrite;
        self
    }
}
