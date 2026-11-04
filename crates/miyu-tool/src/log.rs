//! 这个会话日志的只读入口（`docs/blueprint/tools/interface.md`，施工 6-4）：`history` 翻会话自己的记录用；订阅时补发的那一截
//! 也照它读（施工 3-8 六补，照序号读一截的施工 V-37 下）；`history` 只解要的种类（施工 V-49）。
//!
//! 工具不知道日志存在哪、怎么存：执行器照会话的目录造一个 [`ReadLog`]，包成 [`Log`] 交给这次调用。

use std::fmt;
use std::sync::Arc;

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;

/// 一段一段读这个会话的日志，只读。
pub trait ReadLog: Send + Sync {
    /// 照段的先后，读一段交一段给 `each`，它交回 `false` 就不读下去。最后一段末尾没写完的半行跳过，一个字节
    /// 都不写：会话正在往里写。
    ///
    /// # Errors
    ///
    /// 读不了、日志坏了：原因，给她看。
    fn read(&self, each: &mut dyn FnMut(Vec<Event>) -> bool) -> Result<(), String>;

    /// 照先后读第 `from` 条到第 `upto` 条（施工 V-37 下：订阅时补发的那一截），一块块交给 `each`，它交回 `false` 就不读
    /// 下去；读过 `upto` 就停，后面的不读。最后一段末尾没写完的半行同 [`ReadLog::read`] 跳过。
    ///
    /// 默认照 [`ReadLog::read`] 一段段读、只交这一截。照会话目录读的（执行器造的那一个）照序号定位着读，`from` 以前的不解、
    /// 不自检（`store.md` 第 7 条的 `read_range`）。
    ///
    /// # Errors
    ///
    /// 同 [`ReadLog::read`]。
    fn read_range(
        &self,
        from: Seq,
        upto: Seq,
        each: &mut dyn FnMut(Vec<Event>) -> bool,
    ) -> Result<(), String> {
        self.read(&mut |segment| {
            let past = segment.last().is_some_and(|event| event.seq >= upto);
            let kept: Vec<Event> = segment
                .into_iter()
                .filter(|event| event.seq >= from && event.seq <= upto)
                .collect();
            (kept.is_empty() || each(kept)) && !past
        })
    }

    /// 同 [`ReadLog::read_range`]，只是每一行只解 `only` 里的种类（施工 V-49：`history` 用，`tools/history.md`「怎么走」
    /// 第 3 条）：外壳、`only` 里的、有效历史照种类认的那几种全解，`message.assistant`、`model.called` 只解撤销看的几格，别的
    /// 种类的 `body` 不解、读成不认识的种类（内核的 `Event::from_line_only`）。照先后交给留着一切的有效历史算出来留下的事件
    /// 和全解的一样。
    ///
    /// 默认照 [`ReadLog::read_range`] 全解（测试里的假日志）：全解读得成的，只解要的种类也读得成，交出去的要的种类一样。照会话
    /// 目录读的（执行器、会话表造的）照存储的 `read_range_only` 读：自检、报的错同 `read_range`，只是不要的种类的 `body` 合
    /// JSON、只是不合那一种的写法的不报（2026-11-03 项目主人拍板接受，2026-11-05 批 `history` 也这样）。
    ///
    /// # Errors
    ///
    /// 同 [`ReadLog::read`]。
    fn read_range_only(
        &self,
        from: Seq,
        upto: Seq,
        only: &[&str],
        each: &mut dyn FnMut(Vec<Event>) -> bool,
    ) -> Result<(), String> {
        let _ = only;
        self.read_range(from, upto, each)
    }
}

/// 交给一次调用的日志入口；克隆出来的是同一个。
#[derive(Clone)]
pub struct Log(Arc<dyn ReadLog>);

impl Log {
    /// 包一个 [`ReadLog`]。
    pub fn new(read: impl ReadLog + 'static) -> Log {
        Log(Arc::new(read))
    }

    /// 同 [`ReadLog::read`]。
    ///
    /// # Errors
    ///
    /// 同 [`ReadLog::read`]。
    pub fn read(&self, mut each: impl FnMut(Vec<Event>) -> bool) -> Result<(), String> {
        self.0.read(&mut each)
    }

    /// 同 [`ReadLog::read_range`]。
    ///
    /// # Errors
    ///
    /// 同 [`ReadLog::read`]。
    pub fn read_range(
        &self,
        from: Seq,
        upto: Seq,
        mut each: impl FnMut(Vec<Event>) -> bool,
    ) -> Result<(), String> {
        self.0.read_range(from, upto, &mut each)
    }

    /// 同 [`ReadLog::read_range_only`]。
    ///
    /// # Errors
    ///
    /// 同 [`ReadLog::read`]。
    pub fn read_range_only(
        &self,
        from: Seq,
        upto: Seq,
        only: &[&str],
        mut each: impl FnMut(Vec<Event>) -> bool,
    ) -> Result<(), String> {
        self.0.read_range_only(from, upto, only, &mut each)
    }
}

impl fmt::Debug for Log {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Log")
    }
}

/// 两个入口比的是不是同一个：[`crate::Call`] 照格子比较时用。
impl PartialEq for Log {
    fn eq(&self, other: &Log) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Log {}
