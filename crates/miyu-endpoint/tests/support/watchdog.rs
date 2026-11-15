//! 看门的（施工 RB-1）：一条测试连运行时的收尾一起限时，到了还没完的在标准错误上说卡在哪一步，整个测试程序退出。
//!
//! 测试里的等都带了时限（`Client::call`、`until_listed` 六十秒），可都是 tokio 的计时：运行时的线程被同步地堵住了（一把锁、一个
//! 同步的等），计时就不走，时限一个都不到；`#[tokio::test]` 的运行时收尾要等阻塞线程池里的活都做完，那里一直不完的也一样等下去。
//! CI 的 Windows 那格跑满 60 分钟被取消，只留下一句「跑了 60 秒以上」（run 43589383742）。看门的另起一个线程、用真的时钟，不靠
//! 运行时：到了时限在标准错误上说出最后记下的那一步，`exit(101)`（同测试失败的退出码），这一趟跑在几分钟里失败、说得出是哪一步。

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

/// 一条测试的看门的：丢掉就不看了。
pub struct Watchdog {
    step: Arc<Mutex<&'static str>>,
    done: std::sync::mpsc::Sender<()>,
}

impl Watchdog {
    /// 看 `test` 这一条，最多 `limit`。
    pub fn new(test: &'static str, limit: Duration) -> Watchdog {
        let step = Arc::new(Mutex::new("开始"));
        let (done, wait) = std::sync::mpsc::channel::<()>();
        let seen = Arc::clone(&step);
        std::thread::spawn(move || {
            if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = wait.recv_timeout(limit) {
                let at = *seen.lock().unwrap_or_else(PoisonError::into_inner);
                // 直接写标准错误：测试框架接住的是 `eprintln!`（连带测试里起的线程），接住的那一份退出时就丢了。
                let said = format!(
                    "测试 {test} 过了 {} 秒还没完，卡在「{at}」\n",
                    limit.as_secs()
                );
                drop(std::io::Write::write_all(
                    &mut std::io::stderr(),
                    said.as_bytes(),
                ));
                std::process::exit(101);
            }
        });
        Watchdog { step, done }
    }

    /// 到了这一步。
    pub fn step(&self, now: &'static str) {
        *self.step.lock().unwrap_or_else(PoisonError::into_inner) = now;
    }

    /// 在自己的单线程运行时里跑 `test`，连运行时的收尾（等阻塞线程池里的活做完）一起看着。
    pub fn run<F: std::future::Future<Output = ()>>(&self, test: F) {
        self.step("起运行时");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("起得了运行时");
        runtime.block_on(test);
        self.step("运行时收尾");
        drop(runtime);
    }
}

impl Drop for Watchdog {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "看的那一头已经退出了（到了时限）：发不出去也不要紧"
    )]
    fn drop(&mut self) {
        let _ = self.done.send(());
    }
}
