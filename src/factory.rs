use web_sys::wasm_bindgen::JsValue;

pub struct Factory {
    sys: web_sys::IdbFactory,
}

impl Factory {
    pub fn get() -> crate::Result<Factory> {
        let window = web_sys::window().ok_or(crate::Error::NotInBrowser)?;
        let sys = window
            .indexed_db()
            .map_err(|_| crate::Error::IndexedDbDisabled)?
            .ok_or(crate::Error::IndexedDbDisabled)?;
        Ok(Factory { sys })
    }

    pub fn cmp(&self, lhs: &JsValue, rhs: &JsValue) -> crate::Result<std::cmp::Ordering> {
        use std::cmp::Ordering::*;
        self.sys
            .cmp(lhs, rhs)
            .map(|v| match v {
                -1 => Less,
                0 => Equal,
                1 => Greater,
                v => panic!("Unexpected result of IdbFactory::cmp: {v}"),
            })
            .map_err(
                |e| match crate::error::name(&e).as_ref().map(|s| s as &str) {
                    Some("DataError") => crate::Error::InvalidKey,
                    _ => crate::Error::from_js_value(e),
                },
            )
    }

    // TODO: add `databases` once web-sys has it
}
