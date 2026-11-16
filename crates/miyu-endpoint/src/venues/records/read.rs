//! 判官的记录照日志边读边记（施工 V-39 上，`docs/construction/V-39上-判官边读边记.md`）：一遍读下来只留要显示的最后几条、
//! 撤回和引用的编号，不再把整份日志读成事件、整份交给有效历史、一万多行全渲染再挑（`pass.rs`）。撤销过的会话照样交给真的
//! 有效历史算，只是前面谁都碰不到的那一截收下就放掉；放早了的再读一遍；认不得的退回以前的整份读（[`whole`]）。
//!
//! 只解判官要的种类（施工 V-39 上补，`docs/construction/V-39上补-判官只解析要的种类.md`）：会话目录里的日志照
//! [`read_range_only`] 读，判官自己看的（[`ONLY`]）和有效历史照种类认的全解，`message.assistant`、`model.called` 只解撤销看的
//! 几格，别的种类（工具结果、注入的事实……）只认出外壳，正文不解。行完整、序号连着、段名照旧自检；不要的种类正文合 JSON、
//! 不合写法的不报坏了（2026-11-03 项目主人拍板接受），别的照旧。
//!
//! 判官看到的字、`count`、报错和先后都和以前一样：头一遍照旧整份自检（坏了报坏了，没有报没有），然后取钉下的字（读不出的报
//! 内部错），最后才看 `msg`（不在、不是场所人话、撤掉了的报参数错）。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use miyu_assemble::group::{Picked, Records, Window, quoted_as, records_from, records_in};
use miyu_assemble::{ForwardTexts, GroupChat};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History;
use miyu_kernel::id::{ExternalId, Seq};
use miyu_store::log::{Flow, OpenError, read_range_only};

use super::pass::{Pass, start_building};
use super::recent::{Caps, Lately};
use crate::refusal::Refusal;
use crate::sweep::bones::hash;
use crate::sweep::{Ended, Plan, Tuning};

/// 判官自己要看正文的种类（施工 V-39 上补）：第一条（钉下的字照它找）；人话、她发进群里的、撤回（渲染、引用、要判的那一条，
/// `miyu_assemble::group` 的 `on_record`、`quoted_as`、`records_from`）；回合的开头和结尾、撤销、恢复、撤回排着的话、压缩
/// （`pass.rs`、`crate::sweep`）。有效历史照种类认的那几种（撤销查触发时的回报、子会话回报、等的会话空了）
/// [`miyu_kernel::event::Event::from_line_only`] 自己也全解，`message.assistant`、`model.called` 只解撤销看的几格。
pub(super) const ONLY: &[&str] = &[
    "session.created",
    "message.user",
    "venue.delivered",
    "venue.recalled",
    "turn.started",
    "turn.ended",
    "turn.reverted",
    "turn.unreverted",
    "message.withdrawn",
    "context.compacted",
];

/// 读会话日志的两种读法（测试里换成内存里的）。
pub(super) trait Log {
    /// 第 `from` 条到第 `upto` 条，一块块交给 `each`（同 `read_range`）。
    fn range(
        &mut self,
        from: Seq,
        upto: Option<Seq>,
        each: &mut dyn FnMut(Vec<Event>) -> Flow,
    ) -> Result<(), OpenError>;

    /// 整份（同 `read_events`）。
    fn whole(&mut self) -> Result<Vec<Event>, OpenError>;
}

/// 会话目录里的日志：只解 [`ONLY`]（[`read_range_only`]）。
pub(super) struct Dir(pub(super) PathBuf);

impl Log for Dir {
    fn range(
        &mut self,
        from: Seq,
        upto: Option<Seq>,
        each: &mut dyn FnMut(Vec<Event>) -> Flow,
    ) -> Result<(), OpenError> {
        read_range_only(&self.0, from, upto, ONLY, each)
    }

    /// 从第一条读到最后（同 `read_events`，只是照 [`read_range_only`] 只解要的种类）。
    fn whole(&mut self) -> Result<Vec<Event>, OpenError> {
        let mut all = Vec::new();
        read_range_only(&self.0, Seq::FIRST, None, ONLY, |events| {
            all.extend(events);
            Flow::Go
        })?;
        Ok(all)
    }
}

/// 第 `msg` 条照 `window` 的记录：见模块的说明。`texts` 照会话日志的第一条（没有的是空的）交回群会话的字和转发那几行的字。
pub(super) fn judge(
    log: &mut dyn Log,
    msg: Seq,
    window: Window,
    texts: impl FnOnce(Option<&Event>) -> Result<(GroupChat, Option<ForwardTexts>), Refusal>,
    tuning: Tuning,
) -> Result<Records, Refusal> {
    judge_building(log, msg, window, texts, tuning, None).0
}

/// 同 [`judge`]；`build` 有的，同一遍顺手收一份到读到的最后一条为止的记录窗（施工 V-44，`recent.rs`），读完了就交回（这一问
/// 是拒绝码的也交）；头一遍没读完的没有。
pub(super) fn judge_building(
    log: &mut dyn Log,
    msg: Seq,
    window: Window,
    texts: impl FnOnce(Option<&Event>) -> Result<(GroupChat, Option<ForwardTexts>), Refusal>,
    tuning: Tuning,
    build: Option<Caps>,
) -> (Result<Records, Refusal>, Option<Lately>) {
    let mut built = None;
    let records = judge_with(log, (msg, window), texts, tuning, (build, &mut built));
    (records, built)
}

fn judge_with(
    log: &mut dyn Log,
    (msg, window): (Seq, Window),
    texts: impl FnOnce(Option<&Event>) -> Result<(GroupChat, Option<ForwardTexts>), Refusal>,
    tuning: Tuning,
    (build, built): (Option<Caps>, &mut Option<Lately>),
) -> Result<Records, Refusal> {
    let need = window.most();
    let mut pass = start_building(msg, need, Plan::guess(), tuning, build);
    log.range(Seq::FIRST, None, &mut |events| pass.feed(events))
        .map_err(refused)?;
    pass.finish();
    let whole = |log: &mut dyn Log, upto, chat: &GroupChat, forward, built: &mut Option<Lately>| {
        whole(log, (msg, upto), window, (chat, forward), (build, built))
    };
    let (chat, forward) = match texts(pass.settled.first.as_ref()) {
        Ok(texts) => texts,
        Err(refusal) => {
            // 字读不出来的照旧报内部错；读完了的窗照样交回（没骨架认不得的才算读完）。
            if pass.ended == Ended::Done {
                *built = finished(&mut pass);
            }
            return Err(refusal);
        }
    };
    let forward = forward.as_ref();
    let mut pass = match pass.ended {
        Ended::Done => pass,
        Ended::Unsure => return whole(log, pass.last, &chat, forward, built),
        Ended::Again => {
            let mut again = start_building(msg, need, Plan::exact(pass.reaches), tuning, build);
            if let Some(last) = pass.last {
                log.range(Seq::FIRST, Some(last), &mut |events| again.feed(events))
                    .map_err(refused)?;
            }
            again.finish();
            if again.ended != Ended::Done {
                return whole(log, pass.last, &chat, forward, built);
            }
            again.last = pass.last;
            again
        }
    };
    *built = finished(&mut pass);
    let Some(judged) = &pass.settled.judged else {
        return Err(Refusal::BAD_PARAMS);
    };
    let mut read_back = None;
    let quote = match quoted(log, &pass, judged)? {
        Quote::None => None,
        Quote::Kept(k) => Some(&pass.settled.ring[k]),
        Quote::Read(event) => Some(&*read_back.insert(*event)),
        Quote::Lost => return whole(log, pass.last, &chat, forward, &mut None),
    };
    let last: Vec<&Event> = pass.settled.ring.iter().collect();
    let recalled: BTreeMap<&str, &ExternalId> = pass
        .settled
        .recalled
        .iter()
        .map(|(msg, by)| (msg.as_str(), by))
        .collect();
    let picked = Picked {
        judged,
        before: pass.settled.before,
        last: &last,
        recalled: &recalled,
        quote,
    };
    records_from(&picked, window, &chat, forward).ok_or(Refusal::BAD_PARAMS)
}

/// 一遍收齐了：顺手收的记录窗补上第一条、读到的最后一条交出来（施工 V-44）。
fn finished(pass: &mut Pass) -> Option<Lately> {
    let mut lately = pass.settled.lately.take()?;
    lately.first = pass.settled.first.clone().map(Arc::new);
    lately.last = pass.last;
    Some(lately)
}

/// 以前的读法（施工 V-39 上以前），骨架认不得、引用的那一条读不回来时照它：整份读、整份交给留着一切的有效历史。只要头一遍
/// 读到的第 `upto` 条为止（读的时候还在往后写的，后写的不要，同再读的那一遍）。要建记录窗、还没建的（`build`、`built`），照
/// 整份的有效历史收一份。
fn whole(
    log: &mut dyn Log,
    (msg, upto): (Seq, Option<Seq>),
    window: Window,
    (chat, forward): (&GroupChat, Option<&ForwardTexts>),
    (build, built): (Option<Caps>, &mut Option<Lately>),
) -> Result<Records, Refusal> {
    let mut events = log.whole().map_err(refused)?;
    if let Some(upto) = upto {
        events.truncate(events.partition_point(|event| event.seq <= upto));
    }
    let first = events.first().cloned().map(Arc::new);
    let last = events.last().map(|event| event.seq);
    let mut history = History::whole();
    for event in events {
        history.append(event);
    }
    if let Some(caps) = build.filter(|_| built.is_none()) {
        let mut lately = Lately::new(caps);
        lately.settle(history.events());
        lately.first = first;
        lately.last = last;
        *built = Some(lately);
    }
    records_in(&history, msg, window, chat, forward).ok_or(Refusal::BAD_PARAMS)
}

/// 读日志的错照以前报：没有这个会话、日志坏了。
fn refused(error: OpenError) -> Refusal {
    match error {
        OpenError::Missing(_) => Refusal::NOT_FOUND,
        error => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "venue records not read");
            Refusal::of_log(&error)
        }
    }
}

/// 引用的那一条在哪。
#[derive(Debug, PartialEq)]
pub(super) enum Quote {
    /// 不引用的、它以前没有那个编号的。
    None,
    /// 在留着的最后几条里，第几条。
    Kept(usize),
    /// 不在，照序号读回来的。
    Read(Box<Event>),
    /// 留着的、读回来的对不上（编号的哈希撞了；日志只追加，读回来的不该对不上）。
    Lost,
}

/// 要判的那一条引用的：它以前最后一条那个编号的（照哪一条记的是编号的哈希），在留着的最后几条里就用，不在的照序号读回那一条
/// （只解那一窗）；对不上编号的（哈希撞了）退回整份读。
pub(super) fn quoted(log: &mut dyn Log, pass: &Pass, judged: &Event) -> Result<Quote, Refusal> {
    let reply_to = match &judged.body {
        Body::MessageUser(said) => said
            .venue
            .as_ref()
            .and_then(|venue| venue.reply_to.as_deref()),
        _ => None,
    };
    let Some(seq) = reply_to.and_then(|reply_to| pass.settled.quotes.get(&hash(reply_to)).copied())
    else {
        return Ok(Quote::None);
    };
    if let Ok(k) = pass
        .settled
        .ring
        .binary_search_by_key(&seq, |event| event.seq)
    {
        return Ok(if quoted_as(&pass.settled.ring[k]) == reply_to {
            Quote::Kept(k)
        } else {
            Quote::Lost
        });
    }
    let mut found = None;
    log.range(seq, Some(seq), &mut |events| {
        found = events.into_iter().find(|event| event.seq == seq);
        Flow::Stop
    })
    .map_err(refused)?;
    Ok(match found {
        Some(event) if quoted_as(&event) == reply_to => Quote::Read(Box::new(event)),
        _ => Quote::Lost,
    })
}
