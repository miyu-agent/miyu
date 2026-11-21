//! 让一段代码单独成一个函数（施工 LW-8）：核心的发行程序按执行顺序排函数，执行过的函数整个留在内存里
//! （`internal/blueprint/perf.md`「排函数」）。只有一处调用的函数、只等一次的 future 会被编译器并进调用它的那一个，照种类、
//! 照方法分派的大 `match` 就成了一个几十 kB 的函数，走到其中一支也算执行过整块。分派的每一支、不常走的那一截包一层，各是
//! 一个函数，只有走到的留下。
//!
//! 这里只是机制，不碰 I/O；核心、端点、组装这些层都照它用。

use std::pin::Pin;

/// 照 `f` 办，`f` 的身子单独成一个不内联的函数。
#[inline(never)]
pub fn sync<R>(f: impl FnOnce() -> R) -> R {
    f()
}

/// 把 `future` 放到堆上、当成 `dyn Future` 交回：经虚表调，它的 `poll` 不再并进等它的那一个 async 函数。
pub fn future<'a, T>(
    future: impl Future<Output = T> + Send + 'a,
) -> Pin<Box<dyn Future<Output = T> + Send + 'a>> {
    Box::pin(future)
}
