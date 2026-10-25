//! A running listener, on whichever executor this target has.
//!
//! Natively a listener is a Tokio task. In a browser there is no Tokio runtime to spawn onto —
//! nothing can drive one without blocking the page — so the listener goes to the browser's own
//! event loop instead, and a oneshot channel stands in for the join.

use std::future::Future;

/// A listener that has been started. Await [`ListenerHandle::finished`] to learn it has ended.
pub struct ListenerHandle(Inner);

#[cfg(not(target_arch = "wasm32"))]
type Inner = tokio::task::JoinHandle<()>;

#[cfg(target_arch = "wasm32")]
type Inner = tokio::sync::oneshot::Receiver<()>;

impl ListenerHandle {
    /// Start `task` on Tokio.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn<F>(task: F) -> Self
    where
        F: Future<Output = ()> + Send + 'static,
    {
        Self(tokio::spawn(task))
    }

    /// Start `task` on the browser's event loop.
    #[cfg(target_arch = "wasm32")]
    pub fn spawn<F>(task: F) -> Self
    where
        F: Future<Output = ()> + 'static,
    {
        let (done, finished) = tokio::sync::oneshot::channel();
        wasm_bindgen_futures::spawn_local(async move {
            task.await;
            let _ = done.send(());
        });
        Self(finished)
    }

    /// Resolves once the listener has ended, however it ended.
    pub async fn finished(self) {
        let _ = self.0.await;
    }
}
