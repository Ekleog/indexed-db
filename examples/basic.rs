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
        .open("database", 1, |evt| async move {
            let db = evt.database();
            let store = db.build_object_store("store").auto_increment().create()?;

            // You can also add objects from this callback
            store.add(&JsString::from("foo")).await?;

            Ok(())
        })
        .await
        .context("creating the 'database' IndexedDB")?;

    // In a transaction, add two records
    db.transaction(&["store"])
        .rw()
        .run(|t| async move {
            let store = t.object_store("store")?;
            store.add(&JsString::from("bar")).await?;
            store.add(&JsString::from("baz")).await?;
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
            store.add(&JsString::from("quux")).await?;
            if store.count().await? > 3 {
                // Oops! In this example, we have 4 items by this point
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
            assert_eq!(num_items, 3);
            Ok(())
        })
        .await?;

    Ok(())
}

use wasm_bindgen_test::*;
wasm_bindgen_test_configure!(run_in_browser);
#[wasm_bindgen_test]
async fn test() {
    example().await.unwrap()
}

fn main() {}
