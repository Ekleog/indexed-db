# indexed-db

Bindings for IndexedDB, that default transaction to aborting.

## Why yet another IndexedDB crate?

As of the time of my writing this crate, the alternatives have the default IndexedDB behavior of transaction committing. This is because IndexedDB transactions have strange committing semantics: they commit as soon as the application returns to the event loop without an ongoing request.

This crate forces your transactions to respect the IndexedDB requirements, so as to make it possible to abort transactions upon errors, rather than having them auto-commit.

## Error handling

This crate uses an `Error<E>` type. Most of the functions not designed to be called inside a transaction return `Error`, one variant of this type that does not have any `E` payload.

However, in order to make transactions easy to write, the callback that you need to provide to `TransactionBuilder::run` returns `Result<T, Error<E>>` where both `T` and `E` are user-defined types. This is to make it easy for you to use your own error type. `E` will be automatically wrapped into `Error<E>`, and can be unwrapped by matching the error with `Error::User(_)`.

## Example

```rust
use anyhow::Context;
use indexed_db::Factory;
use web_sys::js_sys::JsString;

async fn example() -> anyhow::Result<()> {
    // Obtain the database builder
    // This database builder will let us easily use custom errors of type
    // `std::io::Error`.
    let factory = Factory::<std::io::Error>::get().context("opening IndexedDB")?;

    // Open the database, creating it if needed
    let db = factory
        .open("database", 1, |evt| {
            let db = evt.database();
            db.build_object_store("store").auto_increment().create()?;
            Ok(())
        })
        .await
        .context("creating the 'database' IndexedDB")?;

    // In a transaction, add two records
    db.transaction(&["store"])
        .rw()
        .run(|t| async move {
            let store = t.object_store("store")?;
            store.add(&JsString::from("foo")).await?;
            store.add(&JsString::from("bar")).await?;
            Ok(())
        })
        .await?;

    // In another transaction, read the first record
    db.transaction(&["store"])
        .run(|t| async move {
            let data = t.object_store("store")?.get_all(Some(1)).await?;
            if data.len() != 1 {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Unexpected data length",
                ))?;
            }
            Ok(())
        })
        .await?;

    // If we return `Err` from a transaction, then it will abort
    db.transaction(&["store"])
        .rw()
        .run(|t| async move {
            let store = t.object_store("store")?;
            store.add(&JsString::from("baz")).await?;
            if store.count().await? > 2 {
                // Oops! In this example, we have 3 items by this point
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Too many objects in store",
                ))?;
            }
            Ok(())
        })
        .await
        .unwrap_err();

    // And no write will have happened
    db.transaction(&["store"])
        .run(|t| async move {
            let num_items = t.object_store("store")?.count().await?;
            assert_eq!(num_items, 2);
            Ok(())
        })
        .await?;

    Ok(())
}
```
