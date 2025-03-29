//! This module holds all the `unsafe` implementation details of `transaction`.
//!
//! The API exposed from here is entirely safe, and this module's code should be properly audited.

use std::{
    cell::{Cell, OnceCell},
    panic::AssertUnwindSafe,
    rc::{Rc, Weak},
};

use futures_util::FutureExt as _;

use super::{runner::poll_it, RunnableTransaction};

struct DropFlag(Rc<Cell<bool>>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

pub struct ScopeCallback<Args> {
    state: Rc<OnceCell<Weak<RunnableTransaction<'static>>>>,
    _dropped: DropFlag,
    maker: Box<dyn 'static + FnOnce(Args) -> RunnableTransaction<'static>>,
}

impl<Args> ScopeCallback<Args> {
    pub fn run(self, args: Args) {
        let made_state = Rc::new((self.maker)(args));
        let _ = self.state.set(Rc::downgrade(&made_state));
        poll_it(&made_state);
    }
}

/// Panics and aborts the whole process if the transaction is not dropped before the end of `scope`
pub async fn extend_lifetime_to_scope_and_run<'scope, MakerArgs, ScopeRet>(
    maker: Box<dyn 'scope + FnOnce(MakerArgs) -> RunnableTransaction<'scope>>,
    scope: impl 'scope + AsyncFnOnce(ScopeCallback<MakerArgs>) -> ScopeRet,
) -> ScopeRet {
    // SAFETY: We're extending the lifetime of `maker` as well as its return value to `'static`.
    // This is safe because the `RunnableTransaction` is not stored anywhere else, and it will be dropped
    // before the end of the enclosing `extend_lifetime_to_scope_and_run` call, at the `Weak::strong_count` check.
    // If it is not, we'll panic and abort the whole process.
    // `'scope` is also guaranteed to outlive `extend_lifetime_to_scope_and_run`.
    // Finally, `maker` itself is guaranteed to not escape `'scope` because it can only be consumed by `run`,
    // and the `ScopeCallback` itself is guaranteed to not escape `'scope` thanks to the check on `dropped`.
    let maker: Box<dyn 'static + FnOnce(MakerArgs) -> RunnableTransaction<'static>> =
        unsafe { std::mem::transmute(maker) };

    let state = Rc::new(OnceCell::new());
    let dropped = Rc::new(Cell::new(false));
    let callback = ScopeCallback {
        state: state.clone(),
        _dropped: DropFlag(dropped.clone()),
        maker,
    };
    let result = AssertUnwindSafe((scope)(callback)).catch_unwind().await;
    if !dropped.get() {
        let _ = std::panic::catch_unwind(|| {
            panic!("Bug in the indexed-db crate: the ScopeCallback was not consumed before the end of its logical lifetime")
        });
        std::process::abort();
    }
    if let Some(state) = state.get() {
        if Weak::strong_count(&state) != 0 {
            // Make sure that regardless of what the user could be doing, if we're overextending the lifetime we'll panic and abort
            //
            // Note: we know this won't spuriously hit because:
            // - we're using `Rc`, so every `RunnableTransaction` operation is single-thread anyway
            // - when the scope completes, `finished_rx` will have resolved
            // - if `finished_tx` has been written to, it means that the `RunnableTransaction` has been dropped
            // Point 2 is enforced outside of the unsafe jar, but it's fine considering it will only result in a spurious panic/abort
            let _ = std::panic::catch_unwind(|| {
                panic!("Bug in the indexed-db crate: the transaction was not dropped before the end of its lifetime")
            });
            std::process::abort();
        }
    }
    match result {
        Ok(result) => result,
        Err(err) => std::panic::resume_unwind(err),
    }
}
