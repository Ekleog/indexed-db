pub struct Factory {
    // TODO
}

impl Factory {
    pub fn get() -> crate::Result<Factory> {
        let _window = web_sys::window().ok_or(crate::Error::NotInBrowser)?;
        // TODO
        Ok(Factory {})
    }
}
