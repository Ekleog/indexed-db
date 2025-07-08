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

pub use cursor::{Cursor, CursorBuilder, CursorDirection};
pub use database::{Database, OwnedDatabase};
pub use error::{Error, Result};
pub use factory::{Factory, ObjectStoreBuilder, VersionChangeEvent};
pub use index::Index;
pub use object_store::{IndexBuilder, ObjectStore};
pub use transaction::{Transaction, TransactionBuilder};

const POLLED_FORBIDDEN_THING_PANIC: &str = "Transaction blocked without any request under way.
The developer probably called .await on something that is not an indexed-db-provided future inside a transaction.
This would lead the transaction to be committed due to IndexedDB semantics.";
