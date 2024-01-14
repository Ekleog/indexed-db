# indexed-db

Bindings for IndexedDB, that default transaction to aborting.

## Why yet another IndexedDB crate?

As of the time of my writing this crate, the alternatives have the default IndexedDB behavior of transaction committing. This is because IndexedDB transactions have strange committing semantics: they commit as soon as the application returns to the event loop without an ongoing request.

This crate forces your transactions to respect the IndexedDB requirements, so as to make it possible to abort transactions upon errors, rather than having them auto-commit.

## Error handling

This crate uses an `Error<Err>` type. The `Err` generic argument is present on basically all the structs exposed by this crate. It is the type of users in code surrounding `indexed-db` usage, for convenience.

In particular, if you ever want to recover one of your own errors (of type `Err`) that went through `indexed-db` code, you should just match the error with `Error::User(_)`, and you will be able to recover your own error details.

On the other hand, when one of your callbacks wants to return an error of your own type through `indexed-db`, it can just use the `From<Err> for Error<Err>` implementation. This is done automatically by the `?` operator, or can be done manually for explicit returns with `return Err(e.into());`.

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
