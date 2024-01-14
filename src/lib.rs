// Internal helper
macro_rules! error_name {
    ($v:expr) => {
        crate::error::name($v).as_ref().map(|s| s as &str)
    };
}

mod database;
mod error;
mod factory;
mod index;
mod object_store;
mod transaction;
mod utils;

pub use database::{Database, ObjectStoreBuilder, ObjectStoreConfigurator};
pub use error::{Error, Result};
pub use factory::{Factory, VersionChangeEvent};
pub use index::Index;
pub use object_store::ObjectStore;
pub use transaction::{Transaction, TransactionBuilder};
