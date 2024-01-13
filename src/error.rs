pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Not running in a browser window")]
    NotInBrowser,

    #[error("IndexedDB is disabled")]
    IndexedDbDisabled,
}
