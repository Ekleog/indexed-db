use indexed_db::{Error, Factory};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::wasm_bindgen::JsValue;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn smoke_test() {
    tracing_wasm::set_as_global_default();
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    // Factory::get
    let factory = Factory::get().unwrap();

    // Factory::cmp
    assert_eq!(
        factory
            .cmp(&JsValue::from_str("foo"), &JsValue::from_str("bar"))
            .unwrap(),
        std::cmp::Ordering::Greater
    );
    assert!(matches!(
        factory.cmp(&JsValue::TRUE, &JsValue::FALSE),
        Err(Error::InvalidKey),
    ));

    // Factory::delete_database
    factory.delete_database("foo").await.unwrap();

    // Factory::open
    factory.open("foo", 2, |_| Ok(())).await.unwrap();
    factory.open("foo", 1, |_| Ok(())).await.unwrap_err();

    // Database::build_object_store
    factory
        .open("bar", 1, |evt| {
            let db = evt.database();
            db.build_object_store("objects").create()?;
            db.build_object_store("things")
                .key_path(&["foo", "bar"])
                .create()?;
            db.build_object_store("stuffs").auto_increment().create()?;
            Ok(())
        })
        .await
        .unwrap();
}
