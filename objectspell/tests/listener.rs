//! A started listener can be waited on, however it ends.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use objectspell::ListenerHandle;

#[tokio::test]
async fn finished_resolves_after_the_task_has_run() {
    let ran = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&ran);

    let handle = ListenerHandle::spawn(async move {
        tokio::task::yield_now().await;
        flag.store(true, Ordering::SeqCst);
    });
    handle.finished().await;

    assert!(ran.load(Ordering::SeqCst));
}

#[tokio::test]
async fn finished_resolves_even_if_the_task_panicked() {
    let handle = ListenerHandle::spawn(async { panic!("listener failed") });

    tokio::time::timeout(std::time::Duration::from_secs(5), handle.finished())
        .await
        .expect("finished() resolves after a panic");
}
