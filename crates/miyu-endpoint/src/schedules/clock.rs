//! 唤醒表用的钟（施工 J-1 上）：现在几点、睡到某一刻。核心用真的钟；测试换一个手拨的钟（[`Clock`] 是公开的，测试那边自己写），
//! 不真等。

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use miyu_kernel::time::Timestamp;

/// 睡到某一刻的 future。
pub type Sleep<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// 钟。
pub trait Clock: Send + Sync {
    /// 现在。
    fn now(&self) -> Timestamp;

    /// 睡到 `at`（已经过了的马上醒）。
    fn sleep_until(&self, at: Timestamp) -> Sleep<'_>;
}

/// 真的钟：系统时间；睡照 tokio 的定时器，一次最多睡一小时再看一眼（改了系统时间、机器睡过一觉，最多晚这么久发现）。
#[derive(Debug, Default)]
pub struct SystemClock;

/// 一次最多睡多久。
const NAP: Duration = Duration::from_secs(3600);

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        crate::sessions::now()
    }

    fn sleep_until(&self, at: Timestamp) -> Sleep<'_> {
        Box::pin(async move {
            loop {
                let left = at.unix_millis() - self.now().unix_millis();
                let Ok(left) = u64::try_from(left) else {
                    return;
                };
                if left == 0 {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(left).min(NAP)).await;
            }
        })
    }
}
