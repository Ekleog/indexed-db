use indexed_db::Factory;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::js_sys::JsString;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
#[should_panic(expected = "Transaction blocked without any request under way")]
async fn other_awaits_panic() {
    let factory = Factory::get().unwrap();

    let db = factory
        .open("baz", 1, |evt| {
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
            rx.await?;
            t.object_store("data")?.add(&JsString::from("bar")).await?;
            Ok(())
        })
        .await
        .unwrap();

    tx.send(()).unwrap();
}
