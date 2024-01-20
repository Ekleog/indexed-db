use indexed_db::{Error, Factory};
use std::rc::Rc;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{
    js_sys::{Function, JsString, Number},
    wasm_bindgen::{closure::Closure, JsCast, JsValue},
    IdbDatabase,
};

wasm_bindgen_test_configure!(run_in_browser);

fn enqueue_main_task_log(s: &str) {
    web_sys::console::log_1(&s.into());
    let _ = web_sys::window()
        .unwrap()
        .scheduler()
        .post_task_with_options(
            &Closure::once_into_js(|| web_sys::console::log_1(&"main thread".into()))
                .unchecked_into(),
            web_sys::SchedulerPostTaskOptions::new().priority(web_sys::TaskPriority::UserBlocking),
        );
}

#[wasm_bindgen_test]
async fn reproducer() {
    let factory = web_sys::window().unwrap().indexed_db().unwrap().unwrap();
    let db_req = Rc::new(factory.open_with_u32("reproducer", 1).unwrap());
    let (tx, rx) = futures_channel::oneshot::channel();
    let db_req2 = db_req.clone();
    let onupgradeneeded = Closure::once(move |_: web_sys::Event| {
        db_req2
            .result()
            .unwrap()
            .dyn_into::<IdbDatabase>()
            .unwrap()
            .create_object_store("example")
            .unwrap();
    });
    db_req.set_onupgradeneeded(Some(
        &onupgradeneeded.as_ref().dyn_ref::<Function>().unwrap(),
    ));
    let onsuccess = Closure::once(|_: web_sys::Event| tx.send(()).unwrap());
    db_req.set_onsuccess(Some(&onsuccess.as_ref().dyn_ref::<Function>().unwrap()));
    rx.await.unwrap();
    let db = db_req.result().unwrap().dyn_into::<IdbDatabase>().unwrap();
    // We now have an IDBDatabase. Start a transaction, and run one request on it.
    let transaction = db.transaction_with_str("example").unwrap();
    let req = transaction
        .object_store("example")
        .unwrap()
        .get(&JsString::from("foo"))
        .unwrap();
    let (tx, rx) = futures_channel::oneshot::channel();
    let onsuccess = Closure::once(|_: web_sys::Event| {
        enqueue_main_task_log("in onsuccess callback");
        tx.send(()).unwrap()
    });
    req.set_onsuccess(Some(&onsuccess.as_ref().dyn_ref::<Function>().unwrap()));
    // getting a non-existent value will succeed with a null result
    enqueue_main_task_log("before rx.await");
    rx.await.unwrap();
    web_sys::console::log_1(&"after rx.await".into());
    // Here, at this await, singlethread will continue rightaway and multithread will go back to event loop once before resuming
    // This results in the line below failing on multithread, but passing on singlethread
    transaction
        .object_store("example")
        .unwrap()
        .get(&JsString::from("bar"))
        .unwrap();
}

/*
#[wasm_bindgen_test]
async fn smoke_test() {
    tracing_wasm::set_as_global_default();

    // Factory::get
    let factory = Factory::<()>::get().unwrap();

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
        .open("foo", 0, |_| async move { Ok(()) })
        .await
        .unwrap_err();
    factory
        .open("foo", 2, |_| async move { Ok(()) })
        .await
        .unwrap();
    factory
        .open("foo", 1, |_| async move { Ok(()) })
        .await
        .unwrap_err();

    // Factory::open_latest_version
    let db = factory.open_latest_version("foo").await.unwrap();
    assert_eq!(db.name(), "foo");
    assert_eq!(db.version(), 2);

    // Database::build_object_store
    let db = factory
        .open("bar", 1, |evt| async move {
            let db = evt.database();
            db.build_object_store("objects").create()?;
            db.build_object_store("things")
                .compound_key_path(&["foo", "bar"])
                .create()?;
            let stuffs = db.build_object_store("stuffs").auto_increment().create()?;
            stuffs.build_index("contents", "").create()?;
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(db.name(), "bar");
    assert_eq!(db.version(), 1);
    assert_eq!(db.object_store_names(), &["objects", "stuffs", "things"]);
    db.close();

    let db = factory
        .open("bar", 2, |evt| async move {
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

            Ok(())
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
        .run(|t| async move {
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
        .run(|t| async move {
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
                all.push(val);
                cursor.delete().await.unwrap();
                cursor.advance(1).await.unwrap();
            }
            assert_eq!(
                all,
                vec![
                    (**JsString::from("value3")).clone(),
                    (**JsString::from("value2")).clone(),
                    (**JsString::from("value1")).clone()
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
    let factory = Factory::get().unwrap();

    let db = factory
        .open("baz", 1, |evt| async move {
            let db = evt.database();
            db.build_object_store("data").auto_increment().create()?;
            Ok(())
        })
        .await
        .unwrap();

    db.transaction(&["data"])
        .rw()
        .run(|t| async move {
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
        .run(|t| async move {
            t.object_store("data")?.add(&JsString::from("baz")).await?;
            Ok::<_, indexed_db::Error<()>>(())
        })
        .await
        .unwrap();

    db.transaction(&["data"])
        .rw()
        .run(|t| async move {
            assert_eq!(t.object_store("data")?.count().await?, 1);
            Ok::<_, indexed_db::Error<()>>(())
        })
        .await
        .unwrap();
}

#[wasm_bindgen_test]
async fn duplicate_insert_returns_proper_error_and_does_not_abort() {
    let factory = Factory::<()>::get().unwrap();

    let db = factory
        .open("quux", 1, |evt| async move {
            let db = evt.database();
            db.build_object_store("data").create()?;
            Ok(())
        })
        .await
        .unwrap();

    db.transaction(&["data"])
        .rw()
        .run(|t| async move {
            t.object_store("data")?
                .add_kv(&JsString::from("key1"), &JsString::from("foo"))
                .await?;
            Ok(())
        })
        .await
        .unwrap();

    db.transaction(&["data"])
        .rw()
        .run(|t| async move {
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
        .run(|t| async move {
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
*/
