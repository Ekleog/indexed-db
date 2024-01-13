pub struct Factory {
    sys: web_sys::IdbFactory,
}

impl Factory {
    pub fn get() -> crate::Result<Factory> {
        let window = web_sys::window().ok_or(crate::Error::NotInBrowser)?;
        let sys = window.indexed_db()
            .map_err(|_| crate::Error::IndexedDbDisabled)?
            .ok_or(crate::Error::IndexedDbDisabled)?;
        Ok(Factory { sys })
    }
}
