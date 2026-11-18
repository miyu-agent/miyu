//! 回合开始问记忆的那几次读（施工 TMP-1 补3）：摘要、人卡、联想在阻塞线程里读，限时过了会话不等（`actor/model.rs` 的
//! `HOOKS`），读照样读完；头一回读一间还要读记忆日志、开库（有的才开）。开始读时登记、读完放开，核心停下时等登记的都读完
//! （[`super::Memory::finish`]）：原来停下以后它们还在碰数据根，测试删数据根时撞上。

use std::sync::{Arc, Condvar, Mutex, MutexGuard};

/// 一个核心一份：还没读完的有几次。
#[derive(Debug, Default)]
pub(crate) struct Reads {
    count: Mutex<usize>,
    done: Condvar,
    /// 测试里让每次读先慢这么多毫秒（`testkit` 的 [`super::Memory::slow_reads`]）：0 是不慢。
    #[cfg(feature = "testkit")]
    slow: std::sync::atomic::AtomicU64,
}

/// 登记着的一次读：放下时读完了。
#[derive(Debug)]
pub(crate) struct Reading {
    reads: Arc<Reads>,
}

impl Reads {
    /// 开始读：登记一次，交回放下时放开的那一份。在派到阻塞线程以前调，交给那边带着。
    pub(crate) fn begin(self: &Arc<Self>) -> Reading {
        *self.lock() += 1;
        Reading {
            reads: Arc::clone(self),
        }
    }

    /// 等这时登记着的都读完：没有的不等。在阻塞线程里调。
    pub(crate) fn settle(&self) {
        let count = self.lock();
        drop(
            self.done
                .wait_while(count, |count| *count > 0)
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }

    fn lock(&self) -> MutexGuard<'_, usize> {
        self.count
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 测试里让往后每次读先慢 `by`。
    #[cfg(feature = "testkit")]
    pub(crate) fn slow(&self, by: std::time::Duration) {
        let millis = u64::try_from(by.as_millis()).unwrap_or(u64::MAX);
        self.slow
            .store(millis, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Reading {
    /// 读以前调：测试里设了慢的先睡一会儿，别的什么都不做。
    pub(crate) fn pace(&self) {
        #[cfg(feature = "testkit")]
        {
            let millis = self.reads.slow.load(std::sync::atomic::Ordering::Relaxed);
            if millis > 0 {
                std::thread::sleep(std::time::Duration::from_millis(millis));
            }
        }
    }
}

impl Drop for Reading {
    fn drop(&mut self) {
        let mut count = self.reads.lock();
        *count = count.saturating_sub(1);
        if *count == 0 {
            self.reads.done.notify_all();
        }
    }
}

#[cfg(test)]
mod tests;
