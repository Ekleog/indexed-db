use indexed_db::{Error, Factory};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{
    js_sys::{JsString, Number},
    wasm_bindgen::JsValue,
};

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
    factory.open("foo", 0, |_| Ok(())).await.unwrap_err();
    factory.open("foo", 2, |_| Ok(())).await.unwrap();
    factory.open("foo", 1, |_| Ok(())).await.unwrap_err();

    // Database::build_object_store
    let db = factory
        .open("bar", 1, |evt| {
            let db = evt.database();
            db.build_object_store("objects").create()?;
            db.build_object_store("things")
                .key_path(&["foo", "bar"])
                .create()?;
            let stuffs = db.build_object_store("stuffs").auto_increment().create()?;
            stuffs.build_index("contents", &[""]).create()?;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(db.name(), "bar");
    assert_eq!(db.version(), 1);
    assert_eq!(db.object_store_names(), &["objects", "stuffs", "things"]);
    db.close();

    let db = factory
        .open("bar", 2, |evt| {
            let db = evt.database();
            db.delete_object_store("things")?;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(db.name(), "bar");
    assert_eq!(db.version(), 2);
    assert_eq!(db.object_store_names(), &["objects", "stuffs"]);

    // Transaction
    db.transaction(&["objects", "stuffs"])
        .rw()
        .run(|t| async move {
            let objects = t.object_store("objects")?;
            let stuffs = t.object_store("stuffs")?;

            // Run one simple addition
            stuffs.add(&JsString::from("foo")).await?;
            assert_eq!(stuffs.count().await?, 1);

            // Run two additions in parallel
            let a = stuffs.add(&JsString::from("bar"));
            let b = objects.add_kv(&JsString::from("key"), &JsString::from("value"));
            let (a, b) = futures::join!(a, b);
            a?;
            b?;
            assert_eq!(stuffs.count().await?, 2);
            assert_eq!(objects.count().await?, 1);
            assert!(objects.contains(&JsString::from("key")).await?);

            Ok::<_, indexed_db::Error<()>>(())
        })
        .await
        .unwrap();
    db.transaction(&["objects", "stuffs"])
        .rw()
        .run(|t| async move {
            let objects = t.object_store("objects")?;
            let stuffs = t.object_store("stuffs")?;

            // Clear objects
            objects.clear().await?;
            assert_eq!(objects.count().await?, 0);

            // Count range
            assert_eq!(
                stuffs
                    .count_in_range(Number::from(2).as_ref()..=Number::from(3).as_ref())
                    .await?,
                1
            );

            Ok::<_, indexed_db::Error<()>>(())
        })
        .await
        .unwrap();
}
