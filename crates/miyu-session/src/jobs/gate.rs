//! 起它的那次调用的结果落盘以前到的，先扣下（施工 RB-1）：后台命令起得快、结束得也快的（`echo`、一起来就报错退出的），等它
//! 的线程报结束、监视攒好的行、到了上限停掉的那一条，可能抢在起它的那次调用把结果（带 `job.started`）交进内核之前到 actor：
//! 跑工具的任务交回结果前还要在阻塞线程里存一趟 blob，机器忙时排在后面。内核那时还不认得这个编号，照账本「对不上一个还没结束
//! 的后台命令的，不理」扔掉，任务表照常拿掉它，之后再没人报：日志里 `job.started` 记着、`job.reported` 永远没有，她不被叫醒，
//! `jobs` 一直显示在跑，长期目标的续轮一直等。
//!
//! 所以照「调用」扣：actor 派一次调用、给它任务端口时记下这次调用还没落盘（[`Gate::opened`]）；端口起的命令记下是哪次调用起的
//! （[`Gate::started`]）；交给 actor 的那一封说的命令，起它的调用还没落盘的扣下（[`Gate::pass`]）。actor 每落一批盘都看一遍
//! （`SessionJobs::note`），记到一条 `tool.result` 就是那次调用落了盘（[`Gate::landed`]）：它起的不再扣，扣着的照到的先后交回去，
//! 这时内核已经认得它们了。那次调用的结果里没有 `job.started` 的（工具崩了、被掐掉）照样交，内核不认，同以前。
//!
//! 调用的结果先落了盘、命令才起来的（被打断了还接着跑完的工具）不扣：起的时候它已经不在 [`Gate::opened`] 记的那一批里了，不然
//! 扣下的永远等不到交出去的那一刻，任务表一直不空。不早到的（结果已经落了盘）照旧当场交，请求里的字一个不变。

use std::collections::{BTreeMap, BTreeSet};

use miyu_kernel::id::{CallId, JobId};

use crate::port::Back;

/// 还没落盘的调用、它们起的命令，和扣下的那几封。拿着 [`super::Shared`] 里它自己的锁改；和任务表的锁一起拿时，先拿任务表的。
#[derive(Debug, Default)]
pub(super) struct Gate {
    /// 给了任务端口、结果还没落盘的调用。
    open: BTreeSet<CallId>,
    /// 起它的那次调用还没落盘的命令：编号 → 那次调用。
    unlanded: BTreeMap<JobId, CallId>,
    /// 扣下的，照到的先后：起它的那次调用、交给 actor 的那一封。测试数它看扣没扣下。
    pub(super) held: Vec<(CallId, Back)>,
}

impl Gate {
    /// 调用 `call` 拿到了任务端口：它的结果落盘以前，它起的命令交来的都扣下。
    pub(super) fn opened(&mut self, call: CallId) {
        self.open.insert(call);
    }

    /// 调用 `call` 起了命令 `job`：那次调用的结果还没落盘的记下；已经落了盘的不记，它交来的当场交。
    pub(super) fn started(&mut self, job: JobId, call: CallId) {
        if self.open.contains(&call) {
            self.unlanded.insert(job, call);
        }
    }

    /// 交给 actor 的一封，说的是命令 `job`：起它的那次调用还没落盘的扣下，交回空的；别的原样交回，当场交。
    pub(super) fn pass(&mut self, job: &JobId, back: Back) -> Option<Back> {
        match self.unlanded.get(job) {
            Some(call) => {
                self.held.push((*call, back));
                None
            }
            None => Some(back),
        }
    }

    /// 调用 `call` 的结果落了盘：它起的不再扣，扣着的照到的先后交回，交给 actor。
    pub(super) fn landed(&mut self, call: CallId) -> Vec<Back> {
        if !self.open.remove(&call) {
            return Vec::new();
        }
        self.unlanded.retain(|_, started| *started != call);
        let (now, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|(started, _)| *started == call);
        self.held = later;
        now.into_iter().map(|(_, back)| back).collect()
    }
}
