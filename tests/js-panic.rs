use anyhow::Context;
use indexed_db::Factory;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::js_sys::JsString;

#[cfg(not(feature = "test-worker"))]
wasm_bindgen_test_configure!(run_in_browser);
#[cfg(feature = "test-worker")]
wasm_bindgen_test_configure!(run_in_worker);

#[wasm_bindgen_test]
#[should_panic(expected = "Transaction blocked without any request under way")]
async fn other_awaits_panic() {
    let factory = Factory::<anyhow::Error>::get().unwrap();

    let db = factory
        .open("baz", 1, |evt| async move {
            let db = evt.database();
            db.build_object_store("data").auto_increment().create()?;
            Ok(())
        })
        .await
        .unwrap();

    let (tx, rx) = futures_channel::oneshot::channel();

    db.transaction(&["data"])
        .rw()
        .run(|t| async move {
            t.object_store("data")?.add(&JsString::from("foo")).await?;
            rx.await.context("awaiting for something external")?;
            t.object_store("data")?.add(&JsString::from("bar")).await?;
            Ok(())
        })
        .await
        .unwrap();

    tx.send(()).unwrap();
}

#[wasm_bindgen_test]
#[should_panic(expected = "Transaction blocked without any request under way")]
async fn await_in_versionchange_panics() {
    let factory = Factory::<anyhow::Error>::get().unwrap();

    let (tx, rx) = futures_channel::oneshot::channel();

    factory
        .open("baz", 1, |evt| async move {
            let db = evt.database();
            db.build_object_store("data").auto_increment().create()?;
            rx.await.context("awaiting for something external")?;
            Ok(())
        })
        .await
        .unwrap();

    tx.send(()).unwrap();
}
