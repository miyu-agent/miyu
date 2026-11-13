//! 在阻塞线程里做完磁盘上的事（`02-内核.md` 第七节「会话 actor 怎么跑」）：读写、同步可能要几十毫秒，
//! 不占异步线程。

use std::time::Instant;

use crate::timing::turn::Waited;

/// 在阻塞线程里做完，交回结果。阻塞的那一头 panic 了，照原样接着 panic：那是 bug。阻塞线程带着派活时的 span，
/// 那边发的行也有会话编号（施工 4-9 再补四上）。
///
/// 回合开始量着的那一段里（施工 PF-3，`timing/turn.rs`），记下这一趟来回等了多久：交出去到那边开始做，加上那边做完到这一头
/// 接着跑。没在量的照原样，不读时钟。
pub(crate) async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    let span = tracing::Span::current();
    let Some(waited) = Waited::current() else {
        return spawned(move || span.in_scope(work)).await;
    };
    let asked = Instant::now();
    let (value, began, ended) = spawned(move || {
        let began = Instant::now();
        let value = span.in_scope(work);
        (value, began, Instant::now())
    })
    .await;
    waited.add(began.saturating_duration_since(asked) + ended.elapsed());
    value
}

/// 交给阻塞线程，等它做完。
async fn spawned<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    match tokio::task::spawn_blocking(work).await {
        Ok(value) => value,
        Err(error) => std::panic::resume_unwind(error.into_panic()),
    }
}
