//! 会话放下内存以后还给系统，攒一阵还一次（施工 MEM-3，`docs/blueprint/heap.md`「怎么走」第 2 条）：会话的 actor 结束、载入一个
//! 会话以后都要 trim 一次，内存紧时会话表卸完一批也要一次。
//!
//! 原来每一次当场各排一个阻塞任务 `malloc_trim`。内存紧时一下子退下上千个会话，就是上千次 trim 一个接一个：trim 走堆时拿着
//! arena 的锁，工作线程上的分配、释放都得等它，会话表一个个问退不退跟着慢下来（施工 MEM-3 量过：多线程的运行时、调好分配器，
//! 退下 1000 个会话拿着表的锁 1.2 秒，不各自 trim 的 0.1 秒；卸完再 trim 一次，内存一样回到 138 MiB，`MEM-3-卸会话卸得少.md`）。
//!
//! 所以攒着：头一次要的时候等 [`QUIET`] 再还，这期间又要的都算在这一次里；开始还的那一刻放下记号，之后要的另攒一次。任何一刻
//! 最多一次在等、一次在走。

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// 头一次要以后等多久再还：1 秒。一批退下、载入在这一秒里攒成一次；晚一秒还，内存紧时隔 10 秒才看一次，不碍事。
const QUIET: Duration = Duration::from_secs(1);

/// 有一次排着、还没开始走堆。整个进程一份：堆是整个进程的。
static QUEUED: AtomicBool = AtomicBool::new(false);

/// 过一会儿把堆里空着的还给系统（`miyu_heap::trim`，在阻塞线程里）：已经有一次排着、还没开始的，算在那一次里。要在 tokio 的
/// 运行时里调。
pub fn trim_soon() {
    if QUEUED.swap(true, Ordering::AcqRel) {
        return;
    }
    let queued = Queued;
    drop(tokio::spawn(async move {
        tokio::time::sleep(QUIET).await;
        drop(tokio::task::spawn_blocking(move || {
            // 先放下记号再走堆：这之后放下的内存，走堆时可能已经过了那一块，另攒一次。
            drop(queued);
            miyu_heap::trim()
        }));
    }));
}

/// 排着的那一次：放下时放下记号。等的时候运行时停了（测试里一个测试一个运行时）、阻塞任务排不进去的，也跟着放下，之后要的照样排。
struct Queued;

impl Drop for Queued {
    fn drop(&mut self) {
        QUEUED.store(false, Ordering::Release);
    }
}
