use std::{cell::RefCell, rc::Rc};
use tokio::sync::oneshot;
use web_sys::{
    js_sys::Function,
    wasm_bindgen::{closure::Closure, JsCast},
    IdbRequest,
};

#[derive(Clone)]
struct ResultChan<T> {
    sender: Rc<RefCell<Option<oneshot::Sender<T>>>>,
}

impl<T> ResultChan<T> {
    fn new() -> (ResultChan<T>, oneshot::Receiver<T>) {
        let (tx, rx) = oneshot::channel();
        let sender = Rc::new(RefCell::new(Some(tx)));
        (ResultChan { sender }, rx)
    }

    fn send(self, v: T) {
        if let Err(_) = self
            .sender
            .borrow_mut()
            .take()
            .expect("Trying to send multiple results")
            .send(v)
        {
            panic!("Receiver went away too quickly");
        }
    }
}

pub(crate) async fn generic_request(req: IdbRequest) -> Result<web_sys::Event, web_sys::Event> {
    let (tx, rx) = ResultChan::new();
    let on_success = Closure::once({
        let tx = tx.clone();
        move |v| tx.send(Ok(v))
    });
    let on_error = Closure::once(move |v| tx.send(Err(v)));
    req.set_onsuccess(Some(on_success.as_ref().dyn_ref::<Function>().unwrap()));
    req.set_onerror(Some(on_error.as_ref().dyn_ref::<Function>().unwrap()));
    rx.await.expect("Sender went away without sending a result")
}
