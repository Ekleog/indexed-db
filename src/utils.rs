use futures_channel::oneshot;
use futures_util::future::{self, Either};
use web_sys::{
    js_sys::Function,
    wasm_bindgen::{closure::Closure, JsCast},
    IdbRequest,
};

pub(crate) async fn generic_request(req: IdbRequest) -> Result<web_sys::Event, web_sys::Event> {
    let (success_tx, success_rx) = oneshot::channel();
    let (error_tx, error_rx) = oneshot::channel();

    let on_success = Closure::once(move |v| success_tx.send(v));
    let on_error = Closure::once(move |v| error_tx.send(v));

    req.set_onsuccess(Some(on_success.as_ref().dyn_ref::<Function>().unwrap()));
    req.set_onerror(Some(on_error.as_ref().dyn_ref::<Function>().unwrap()));

    match future::select(success_rx, error_rx).await {
        Either::Left((res, _)) => Ok(res.unwrap()),
        Either::Right((res, _)) => Err(res.unwrap()),
    }
}
