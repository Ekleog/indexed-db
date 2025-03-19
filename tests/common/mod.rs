use indexed_db::{Error, Factory};
use wasm_bindgen_test::wasm_bindgen_test;
use web_sys::{
    js_sys::{JsString, Number, Uint8Array},
    wasm_bindgen::JsValue,
};

async fn time_it<R>(cb: impl std::future::Future<Output = R>) -> (f64, R) {
    let now = {
        use web_sys::{
            js_sys::{global, Reflect},
            wasm_bindgen::JsCast,
            Performance,
        };

        let performance = Reflect::get(&global(), &"performance".into())
            .unwrap()
            .unchecked_into::<Performance>();
        move || performance.now()
    };
    let start_ms = now();
    let ret = cb.await;
    let elapsed_ms = now() - start_ms;
    (elapsed_ms, ret)
}

#[wasm_bindgen_test]
async fn close_and_delete_before_reopen() {
    // tracing_wasm::set_as_global_default();
    // std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    const DATABASE_NAME: &str = "close_and_delete_before_reopen";

    for _ in 0..10 {
        let factory = Factory::get().unwrap();

        let (delete_duration_ms, _) = time_it(async {
            factory.delete_database(DATABASE_NAME).await.unwrap();
        })
        .await;
        // Deleting the database should be almost instantaneous in theory.
        // However this operation will hang as long as the database is still opened,
        // which can last for 10s of seconds if our code forget to close is (in
        // which case the close will only occur when the underlying javascript
        // object got garbage collected...).
        assert!(
            delete_duration_ms < 1000f64,
            "Deleting the database took too long: {}ms",
            delete_duration_ms
        );

        {
            let _db = factory
                .open::<()>(DATABASE_NAME, 1, async move |_| Ok(()))
                .await
                .unwrap();
            // Here the database wrapper got dropped which should trigger database close.
        }
    }
}

#[wasm_bindgen_test]
async fn into_manual_close() {
    // tracing_wasm::set_as_global_default();
    // std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    const DATABASE_NAME: &str = "into_manual_close";

    let factory = Factory::get().unwrap();
    factory.delete_database(DATABASE_NAME).await.unwrap();

    let manual_close_db = {
        let db = factory
            .open::<()>(DATABASE_NAME, 1, async move |evt| {
                evt.build_object_store("objects")
                    .auto_increment()
                    .create()?;
                Ok(())
            })
            .await
            .unwrap();
        db.into_manual_close()
    };

    // `db` has been dropped, but we should still be able to use `manual_close_db`.
    manual_close_db
        .transaction(&["objects"])
        .run::<_, ()>(async move |t| {
            let objects = t.object_store("objects")?;
            assert!(objects.get(&JsString::from("nokey")).await?.is_none());
            Ok(())
        })
        .await
        .unwrap();

    manual_close_db.close();

    // Now the database is fully closed
    let outcome = manual_close_db
        .transaction(&["objects"])
        .run(async move |_| -> Result<(), Error<()>> {
            unreachable!("Database is closed");
        })
        .await;
    assert!(
        matches!(outcome, Err(Error::DatabaseIsClosed)),
        "Unexpected outcome: {:?}",
        outcome
    );
}

#[wasm_bindgen_test]
async fn smoke_test() {
    // tracing_wasm::set_as_global_default();
    // std::panic::set_hook(Box::new(console_error_panic_hook::hook));

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
    factory
        .open::<()>("foo", 0, async move |_| Ok(()))
        .await
        .unwrap_err();
    factory
        .open::<()>("foo", 2, async move |_| Ok(()))
        .await
        .unwrap();
    factory
        .open::<()>("foo", 1, async move |_| Ok(()))
        .await
        .unwrap_err();

    // Factory::open_latest_version
    let db = factory.open_latest_version("foo").await.unwrap();
    assert_eq!(db.name(), "foo");
    assert_eq!(db.version(), 2);

    // Database::build_object_store
    let db = factory
        .open::<()>("bar", 1, async move |evt| {
            evt.build_object_store("objects").create()?;
            evt.build_object_store("things")
                .compound_key_path(&["foo", "bar"])
                .create()?;
            let stuffs = evt.build_object_store("stuffs").auto_increment().create()?;
            stuffs.build_index("contents", "").create()?;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(db.name(), "bar");
    assert_eq!(db.version(), 1);
    assert_eq!(db.object_store_names(), &["objects", "stuffs", "things"]);
    drop(db); // Close the database

    let db = factory
        .open::<()>("bar", 2, async move |evt| {
            evt.delete_object_store("things")?;
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
        .run::<_, ()>(async move |t| {
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

            Ok(())
        })
        .await
        .unwrap();
    db.transaction(&["objects", "stuffs"])
        .rw()
        .run::<_, ()>(async move |t| {
            let objects = t.object_store("objects")?;
            let stuffs = t.object_store("stuffs")?;

            // Clear objects
            objects.clear().await?;
            assert_eq!(objects.count().await?, 0);

            // Count range
            assert_eq!(
                stuffs
                    .count_in(Number::from(2).as_ref()..=Number::from(3).as_ref())
                    .await?,
                1
            );

            // Delete
            stuffs
                .delete_range(Number::from(2).as_ref()..=Number::from(3).as_ref())
                .await?;
            assert_eq!(stuffs.count().await?, 1);
            stuffs.delete(&Number::from(1)).await?;
            assert_eq!(stuffs.count().await?, 0);

            Ok(())
        })
        .await
        .unwrap();
    db.transaction(&["objects"])
        .rw()
        .run::<_, ()>(async move |t| {
            let objects = t.object_store("objects")?;

            // Get
            objects
                .add_kv(&JsString::from("key"), &JsString::from("value"))
                .await?;
            assert_eq!(
                objects.get(&JsString::from("key")).await?.unwrap(),
                **JsString::from("value")
            );
            assert!(objects.get(&JsString::from("nokey")).await?.is_none());
            assert_eq!(
                objects
                    .get_first_in(..JsString::from("zzz").as_ref())
                    .await?
                    .unwrap(),
                **JsString::from("value")
            );
            assert_eq!(
                objects.get_all(None).await?,
                vec![(**JsString::from("value")).clone()],
            );
            assert_eq!(
                objects
                    .get_all_in(JsString::from("zzz").as_ref().., None)
                    .await?,
                Vec::<JsValue>::new(),
            );

            Ok(())
        })
        .await
        .unwrap();
    db.transaction(&["stuffs"])
        .rw()
        .run::<_, ()>(async move |t| {
            let stuffs = t.object_store("stuffs")?;

            // Index
            stuffs.add(&JsString::from("value3")).await?;
            stuffs.put(&JsString::from("value2")).await?;
            stuffs.add(&JsString::from("value1")).await?;
            assert_eq!(
                stuffs.get_all(None).await?,
                vec![
                    (**JsString::from("value3")).clone(),
                    (**JsString::from("value2")).clone(),
                    (**JsString::from("value1")).clone()
                ]
            );
            assert_eq!(
                stuffs.index("contents").unwrap().get_all(None).await?,
                vec![
                    (**JsString::from("value1")).clone(),
                    (**JsString::from("value2")).clone(),
                    (**JsString::from("value3")).clone()
                ]
            );

            // Cursors
            let mut all = Vec::new();
            let mut cursor = stuffs.cursor().open().await.unwrap();
            while let Some(val) = cursor.value() {
                all.push((cursor.primary_key().unwrap(), val));
                cursor.delete().await.unwrap();
                cursor.advance(1).await.unwrap();
            }
            assert_eq!(
                all,
                vec![
                    (JsValue::from(3), (**JsString::from("value3")).clone()),
                    (JsValue::from(4), (**JsString::from("value2")).clone()),
                    (JsValue::from(5), (**JsString::from("value1")).clone())
                ]
            );
            assert_eq!(stuffs.count().await.unwrap(), 0);

            Ok(())
        })
        .await
        .unwrap();
}

#[wasm_bindgen_test]
async fn auto_rollback() {
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

    db.transaction(&["data"])
        .rw()
        .run::<_, ()>(async move |t| {
            t.object_store("data")?.add(&JsString::from("foo")).await?;
            t.object_store("data")?.add(&JsString::from("bar")).await?;
            if true {
                // Something went wrong!
                Err::<(), _>(())?;
            }
            Ok(())
        })
        .await
        .unwrap_err();

    db.transaction(&["data"])
        .rw()
        .run::<_, ()>(async move |t| {
            t.object_store("data")?.add(&JsString::from("baz")).await?;
            Ok::<_, indexed_db::Error<()>>(())
        })
        .await
        .unwrap();

    db.transaction(&["data"])
        .rw()
        .run::<_, ()>(async move |t| {
            assert_eq!(t.object_store("data")?.count().await?, 1);
            Ok::<_, indexed_db::Error<()>>(())
        })
        .await
        .unwrap();
}

#[wasm_bindgen_test]
async fn duplicate_insert_returns_proper_error_and_does_not_abort() {
    let factory = Factory::get().unwrap();

    let db = factory
        .open::<()>("quux", 1, async move |evt| {
            evt.build_object_store("data").create()?;
            Ok(())
        })
        .await
        .unwrap();

    db.transaction(&["data"])
        .rw()
        .run::<_, ()>(async move |t| {
            t.object_store("data")?
                .add_kv(&JsString::from("key1"), &JsString::from("foo"))
                .await?;
            Ok(())
        })
        .await
        .unwrap();

    db.transaction(&["data"])
        .rw()
        .run::<_, ()>(async move |t| {
            assert!(matches!(
                t.object_store("data")?
                    .add_kv(&JsString::from("key1"), &JsString::from("bar"))
                    .await
                    .unwrap_err(),
                indexed_db::Error::AlreadyExists
            ));
            t.object_store("data")?
                .add_kv(&JsString::from("key2"), &JsString::from("baz"))
                .await?;
            Ok(())
        })
        .await
        .unwrap();

    db.transaction(&["data"])
        .rw()
        .run::<_, ()>(async move |t| {
            assert_eq!(
                t.object_store("data")?.get_all_keys(None).await?,
                vec![JsValue::from("key1"), JsValue::from("key2")]
            );
            assert_eq!(
                t.object_store("data")?.get_all(None).await?,
                vec![JsValue::from("foo"), JsValue::from("baz")]
            );
            Ok(())
        })
        .await
        .unwrap();
}

#[wasm_bindgen_test]
async fn typed_array_keys() {
    let factory = Factory::get().unwrap();

    let db = factory
        .open::<()>("db12", 1, async move |evt| {
            evt.build_object_store("data").create()?;
            Ok(())
        })
        .await
        .unwrap();

    db.transaction(&["data"])
        .rw()
        .run::<_, ()>(async move |t| {
            let data = t.object_store("data")?;
            data.add_kv(&Uint8Array::from(&b"key1"[..]), &JsString::from("foo"))
                .await?;
            data.add_kv(&Uint8Array::from(&b"key2"[..]), &JsString::from("bar"))
                .await?;
            data.add_kv(&Uint8Array::from(&b"key3"[..]), &JsString::from("baz"))
                .await?;
            assert_eq!(
                2,
                data.count_in(Uint8Array::from(&b"key2"[..]).as_ref()..)
                    .await?
            );
            assert_eq!(
                1,
                data.count_in(..Uint8Array::from(&b"key2"[..]).as_ref())
                    .await?
            );

            Ok(())
        })
        .await
        .unwrap();
}
