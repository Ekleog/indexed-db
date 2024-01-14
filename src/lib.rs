#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

// Internal helper
macro_rules! error_name {
    ($v:expr) => {
        crate::error::name($v).as_ref().map(|s| s as &str)
    };
}

mod cursor;
mod database;
mod error;
mod factory;
mod index;
mod object_store;
mod transaction;
mod utils;

pub use cursor::{Cursor, CursorDirection, KeyCursor};
pub use database::{Database, ObjectStoreBuilder};
pub use error::{Error, Result};
pub use factory::{Factory, VersionChangeEvent};
pub use index::Index;
pub use object_store::{CursorBuilder as ObjectStoreCursorBuilder, IndexBuilder, ObjectStore};
pub use transaction::{Transaction, TransactionBuilder};
