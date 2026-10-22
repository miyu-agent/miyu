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
    let waited = tokio::time::timeout(WAIT, async {
        loop {
            let log = home.log(session);
            if done(&log) {
                return log;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await;
    waited.unwrap_or_else(|_| panic!("六十秒内磁盘上没等到：{:?}", kinds(&home.log(session))))
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
