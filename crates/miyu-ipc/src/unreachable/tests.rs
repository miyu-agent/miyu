use super::*;
use crate::test_support::Scratch;
use crate::{Lock, open_locked};

/// 起一个核心的样子：拿锁、照这台机器的位置在套接字上等着。
fn listening(root: &DataRoot) -> crate::Opened {
    open_locked(
        root,
        &Dirs::current(),
        Lock::acquire(root).expect("拿得到锁"),
    )
    .expect("开得了")
}

/// 位置、令牌被删了，锁还拿着：算在跑。放了以后不算。
#[tokio::test]
async fn a_held_lock_means_a_core_is_there() {
    let scratch = Scratch::new();
    let root = scratch.root();
    let opened = listening(&root);
    for name in ["socket", "token"] {
        std::fs::remove_file(root.run().join(name)).expect("删得掉");
    }
    assert!(running_unreachable(&root).await.expect("问得出"));
    drop(opened);
    assert!(!running_unreachable(&root).await.expect("问得出"));
}

/// 锁文件也删了（整个 `run/` 删了的样子，套接字不在里面）：照算出来的位置连得上，算在跑。
#[tokio::test]
async fn a_core_still_listening_where_it_should_counts() {
    let scratch = Scratch::new();
    let root = scratch.root();
    let opened = listening(&root);
    for name in ["core.lock", "socket", "token"] {
        std::fs::remove_file(root.run().join(name)).expect("删得掉");
    }
    assert!(running_unreachable(&root).await.expect("问得出"));
    drop(opened);
    assert!(!running_unreachable(&root).await.expect("问得出"));
}

/// 从没起过的、走了只留下文件的：不算。
#[tokio::test]
async fn nothing_left_running_is_not_there() {
    let scratch = Scratch::new();
    let root = scratch.root();
    assert!(!running_unreachable(&root).await.expect("问得出"));
    drop(listening(&root));
    assert!(root.run().join("core.lock").is_file(), "留下了锁文件");
    assert!(!running_unreachable(&root).await.expect("问得出"));
}
