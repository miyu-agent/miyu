//! 读会话日志的几样（从 `mod.rs` 挪出来）：等磁盘上的日志满足条件、事件的种类、会话目录的只读入口。

use miyu_kernel::event::Event;
use miyu_kernel::id::SessionId;
use miyu_store::log::read_segments;
use miyu_tool::ReadLog;
use std::path::PathBuf;

use super::{Home, WAIT};

/// 等到磁盘上会话 `session` 的日志满足 `done`，最多 [`WAIT`]；交回那时的日志。
pub async fn until_logged(
    home: &Home,
    session: &SessionId,
    done: impl Fn(&[Event]) -> bool,
) -> Vec<Event> {
    until_logged_within(home, session, WAIT, done).await
}

/// 同 [`until_logged`]，最多等 `wait`（施工 FL-11）：一轮就要走几百步的测试用，机器忙的时候光是一步步排着轮到 CPU 就过了一分钟。
///
/// 每看一次都从头读整份日志：隔多久看一次照等了多久放宽（[`pause`]），等得久的长日志不再每 5 毫秒读一遍。
pub async fn until_logged_within(
    home: &Home,
    session: &SessionId,
    wait: std::time::Duration,
    done: impl Fn(&[Event]) -> bool,
) -> Vec<Event> {
    let started = tokio::time::Instant::now();
    let waited = tokio::time::timeout(wait, async {
        loop {
            let log = home.log(session);
            if done(&log) {
                return log;
            }
            tokio::time::sleep(pause(started.elapsed())).await;
        }
    })
    .await;
    waited.unwrap_or_else(|_| {
        panic!(
            "{} 秒内磁盘上没等到：{:?}",
            wait.as_secs(),
            kinds(&home.log(session))
        )
    })
}

/// 等了 `waited` 以后隔多久再看一次（施工 FL-11）：等过的二十分之一，至少 5 毫秒、至多 250 毫秒。原来一直 5 毫秒：fork 子代理
/// 走满 500 步的那条测试（两千多条事件）光反复读日志，单跑就从约 5.5 秒拖到 23.7 秒；晚看到的至多多等二十分之一。
fn pause(waited: std::time::Duration) -> std::time::Duration {
    (waited / 20).clamp(
        std::time::Duration::from_millis(5),
        std::time::Duration::from_millis(250),
    )
}

/// 事件的种类，照先后。
pub fn kinds(events: &[Event]) -> Vec<&str> {
    events.iter().map(|event| event.body.kind()).collect()
}

/// 一个会话目录的只读入口（施工 C-4）：和生产里会话表那一头开别的会话日志的办法同一个读法
/// （[`read_segments`]），测试里直接拿会话的真实目录造它，不载入那个会话。
pub struct LogDir(pub PathBuf);

impl ReadLog for LogDir {
    fn read(&self, each: &mut dyn FnMut(Vec<Event>) -> bool) -> Result<(), String> {
        read_segments(&self.0, each).map_err(|error| error.to_string())
    }
}
