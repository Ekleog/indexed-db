use anyhow::Context;
use indexed_db::Factory;
use wasm_bindgen_test::wasm_bindgen_test;
use web_sys::js_sys::JsString;

#[wasm_bindgen_test]
#[should_panic] // For some reason the error message is not detected here, but appears clearly with console_error_panic_hook
async fn other_awaits_panic() {
    // tracing_wasm::set_as_global_default();
    // std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let factory = Factory::get().unwrap();

    let db = factory
        .open::<()>("baz", 1, async move |evt| {
            evt.build_object_store("data").auto_increment().create()?;
            Ok(())
        })
        .await
        .unwrap();

    let (tx, rx) = futures_channel::oneshot::channel();

    db.transaction(&["data"])
        .rw()
        .run::<_, anyhow::Error>(async move |t| {
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
#[should_panic] // For some reason the error message is not detected here, but appears clearly with console_error_panic_hook
async fn await_in_versionchange_panics() {
    // tracing_wasm::set_as_global_default();
    // std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let factory = Factory::get().unwrap();

    let (tx, rx) = futures_channel::oneshot::channel();

    factory
        .open::<anyhow::Error>("baz", 1, async move |evt| {
            evt.build_object_store("data").auto_increment().create()?;
            rx.await.context("awaiting for something external")?;
            Ok(())
        })
        .await
        .unwrap();

    tx.send(()).unwrap();
}
