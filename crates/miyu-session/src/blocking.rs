//! 在阻塞线程里做完磁盘上的事（`02-内核.md` 第七节「会话 actor 怎么跑」）：读写、同步可能要几十毫秒，
//! 不占异步线程。

use std::sync::{Arc, Mutex};
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

/// 在一个单起、做完就退的线程里做完，等那个线程整个退完才交回（施工 LW-10 下：收成槽的历史从日志读回）。读回的事件在那个线程里
/// 分配，线程退的时候它的线程缓存（glibc 的 tcache）还回各自的 arena；放在常驻的阻塞线程里做，那几块空着的会一直钉着那几页
/// （`internal/blueprint/kernel/history.md`「闲下来收成槽」）。外面套一层阻塞任务等它：等的一方不占异步线程，tokio 暂停的
/// 时钟在阻塞任务做着的时候也不自己往前走（测试照它拨钟）。起不了线程的（系统不给）在那个阻塞任务里当场做。
///
/// # Errors
///
/// 做的那一头 panic 了，交回 `Err(())`；带着的东西跟着丢了。
pub(crate) async fn once<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ()> {
    let span = tracing::Span::current();
    let work = Arc::new(Mutex::new(Some(work)));
    let taken = Arc::clone(&work);
    tokio::task::spawn_blocking(move || {
        let run = move || {
            let work = taken.lock().ok().and_then(|mut slot| slot.take());
            work.map(|work| span.in_scope(work))
        };
        let thread = std::thread::Builder::new().name("miyu-once".to_string());
        match thread.spawn(run) {
            Ok(thread) => thread.join().ok().flatten(),
            Err(_) => work
                .lock()
                .ok()
                .and_then(|mut slot| slot.take())
                .map(|work| work()),
        }
    })
    .await
    .ok()
    .flatten()
    .ok_or(())
}

/// 把 `value` 交给一个放完就退的线程放（施工 LW-10 下，同 [`once`] 的道理）：收成槽时拿出来的整条事件、读回来的那一份。起不了
/// 线程的当场放。放完要还给系统的，调的一方另叫 [`crate::heap::trim_soon`]。
pub(crate) fn release<T: Send + 'static>(value: T) {
    let thread = std::thread::Builder::new().name("miyu-release".to_string());
    // 起不了线程时闭包连同 `value` 当场在这里放掉。
    drop(thread.spawn(move || drop(value)));
}
