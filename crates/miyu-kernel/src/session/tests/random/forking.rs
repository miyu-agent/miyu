//! 分叉、载入支线新旧对照（施工 V-39 中，`docs/construction/V-39中-支线底子边读边喂.md`）：以前的整份底子那一套原样抄在
//! `old.rs` 当准绳。父会话是随机输入真跑出来的日志（[`drive`]，撤销、恢复、撤回、压缩及撤销跨压缩、清空、回报、换权限、
//! 换模型、崩了重启……），三百个种子各截十处（在跑一轮当中、一步刚派出去调用的地方、压缩和撤销以后、随便哪里），三千份。
//!
//! 每一份比：分叉要照的几样（含报错）；分叉写下的第一批事件逐字节（普通分叉、fork 子代理、分叉点写错了的）；载入支线
//! 出来的会话和动作（[`Session`] 的 `Debug` 逐字，一块块喂的对不上照整份走的也算），底子一块块喂的、分叉喂好交过来的
//! 两条路都比；父会话没写到分叉点的报错。底子、自己的日志都照随机的块大小喂。

mod old;

use super::*;
use crate::event::Forked;
use crate::session::tests::clear::clear;
use crate::session::tests::revert::{revert, unrevert};
use crate::session::{BaseFeed, ForkFeed, ForkPlan, LoadError};
use crate::testkit::CHILD_SESSION;

/// 走到了哪些情况：查的不是空话。
#[derive(Debug, Default)]
struct Seen {
    /// 比过的父会话日志份数。
    logs: usize,
    /// 普通分叉成了的；分叉点退到在跑的一轮之前的。
    forks: usize,
    before_turn: usize,
    /// fork 子代理成了的；派的调用不对、被拒的。
    within: usize,
    within_refused: usize,
    /// 分叉要照的几样报错的（日志过不了账本、空的）。
    plan_refused: usize,
    /// 底子压缩过的（有效历史不从第 1 条起，喂的时候放掉过）。
    compacted: usize,
    /// 新的一块块喂对不上、照整份走的（旧的没走）；新旧都照整份走的。
    whole_new_only: usize,
    whole_both: usize,
    /// 载入成了的；支线自己清空过、撤过的。
    loaded: usize,
    cleared: usize,
    reverted: usize,
    /// 分叉点写错了、底子没写到分叉点，被拒的。
    wrong_upto: usize,
    short: usize,
}

#[test]
fn forking_and_loading_a_fork_match_the_old_way() {
    let mut seen = Seen::default();
    for seed in 0..300 {
        let log = drive(seed).log();
        let mut dice = Rng(seed ^ 0x0F02_C390);
        for cut in cuts(&log, &mut dice) {
            seen.logs += 1;
            compare(seed, &log[..cut], &mut dice, &mut seen);
        }
    }
    assert!(seen.logs >= 3000, "{seen:?}");
    for (what, count) in [
        ("普通分叉", seen.forks),
        ("退到在跑的一轮之前", seen.before_turn),
        ("fork 子代理", seen.within),
        ("fork 子代理被拒", seen.within_refused),
        ("日志过不了账本", seen.plan_refused),
        ("底子压缩过", seen.compacted),
        ("新的照整份走", seen.whole_new_only + seen.whole_both),
        ("载入", seen.loaded),
        ("支线清空过", seen.cleared),
        ("支线撤过", seen.reverted),
        ("分叉点写错", seen.wrong_upto),
        ("底子没写到分叉点", seen.short),
    ] {
        assert!(count > 0, "一次都没走到「{what}」：{seen:?}");
    }
}

/// 截哪几处（截下的是前几条）：整份、九处随便的，在跑一轮当中、一步刚派出调用、压缩、撤销以后的各优先挑一处。
fn cuts(log: &[Event], dice: &mut Rng) -> Vec<usize> {
    let mut special = [Vec::new(), Vec::new(), Vec::new()];
    for (k, event) in log.iter().enumerate() {
        match &event.body {
            Body::MessageAssistant(message)
                if message
                    .blocks
                    .iter()
                    .any(|block| matches!(block, crate::block::Block::ToolCall(_))) =>
            {
                special[0].push(k + 1);
            }
            Body::ContextCompacted(_) => special[1].push(k + 1),
            Body::TurnReverted(_) | Body::TurnUnreverted(_) | Body::MessageWithdrawn(_) => {
                special[2].push(k + 1);
            }
            _ => {}
        }
    }
    let mut cuts = vec![log.len()];
    for list in &special {
        if !list.is_empty() {
            cuts.push(list[dice.below(list.len() as u64) as usize]);
        }
    }
    while cuts.len() < 10 {
        cuts.push(1 + dice.below(log.len() as u64) as usize);
    }
    cuts
}

/// 父会话的日志 `parent`（截下的前几条）新旧各分叉、载入一遍，一样一样比。
fn compare(seed: u64, parent: &[Event], dice: &mut Rng, seen: &mut Seen) {
    let broken = spoil(parent, dice);
    let parent = &broken[..];
    let ledger = {
        let mut ledger = crate::ledger::Ledger::for_session(session_id());
        for event in parent {
            if ledger.append(event).is_err() {
                break;
            }
        }
        ledger
    };
    // 普通分叉；有等结果的调用的，挑一个派 fork 子代理；偶尔挑一个不在等的。
    let mut calls = vec![None];
    let pending = ledger.pending_calls();
    if !pending.is_empty() {
        calls.push(Some(pending[dice.below(pending.len() as u64) as usize]));
    }
    if dice.below(4) == 0 {
        let at = parent.last().map_or(seq(1), |event| event.seq);
        calls.push(CallId::new(at, 7));
    }
    for call in calls {
        let old_plan = match call {
            Some(call) => old::fork_plan_at(&session_id(), parent, call),
            None => old::fork_plan(&session_id(), parent),
        };
        let (new_plan, fed, settled) = plan(parent, call, dice);
        assert_eq!(
            new_plan,
            old_plan,
            "种子 {seed}，{} 条，{call:?}",
            parent.len()
        );
        let (Ok(plan), Some(fed)) = (old_plan, fed) else {
            match call {
                Some(_) => seen.within_refused += 1,
                None => seen.plan_refused += 1,
            }
            continue;
        };
        let upto = usize::try_from(plan.upto.get()).unwrap();
        assert_eq!(
            settled,
            (1..=plan.upto.get()).collect::<Vec<_>>(),
            "种子 {seed}：交出去记账的正好是到分叉点为止的每一条"
        );
        match call {
            Some(_) => seen.within += 1,
            None => seen.forks += 1,
        }
        if upto < parent.len() && call.is_none() {
            seen.before_turn += 1;
        }
        let base = &parent[..upto];
        fork_both(seed, base, &plan, fed, dice, seen);
    }
}

/// 六份里有一份弄得过不了账本（序号照旧连着，日志读得出）：少一条、一轮的结束换成这一轮里排着的一句（下一轮开不了）、
/// 一条写成别的回合的。
fn spoil(parent: &[Event], dice: &mut Rng) -> Vec<Event> {
    let mut events = parent.to_vec();
    if events.len() < 3 || dice.below(6) != 0 {
        return events;
    }
    let k = 1 + dice.below(events.len() as u64 - 1) as usize;
    match dice.below(3) {
        0 => {
            events.remove(k);
        }
        1 => {
            let ended: Vec<usize> = (1..events.len())
                .filter(|k| matches!(events[*k].body, Body::TurnEnded(_)))
                .collect();
            if let Some(&k) = ended.get(dice.below(ended.len().max(1) as u64) as usize) {
                let line = format!(
                    r#"{{"seq":{},"at":"2026-11-03T08:00:00.000Z","kind":"message.user","turn":{},"by":{{"kind":"person"}},"body":{{"blocks":[]}}}}"#,
                    events[k].seq,
                    events[k].turn.map_or(0, |turn| turn.started().get())
                );
                events[k] = Event::from_line(&line).unwrap();
            }
        }
        _ => events[k].turn = Some(TurnId::new(seq(99_999))),
    }
    events
}

/// 一块块喂 [`ForkFeed`]：交回分叉要照的几样、喂好的底子、交出去记账的序号。
fn plan(
    parent: &[Event],
    call: Option<CallId>,
    dice: &mut Rng,
) -> (Result<ForkPlan, LoadError>, Option<BaseFeed>, Vec<u64>) {
    let mut feed = ForkFeed::new(session_id());
    let mut settled = Vec::new();
    for block in blocks(parent, dice) {
        feed.feed(block, &mut |event| settled.push(event.seq.get()));
    }
    match feed.finish(call, &mut |event| settled.push(event.seq.get())) {
        Ok((plan, fed)) => (Ok(plan), Some(fed), settled),
        Err(error) => (Err(error), None, settled),
    }
}

/// 照随机的块大小切开（一条一块、几条一块、整份一块）。
fn blocks(events: &[Event], dice: &mut Rng) -> Vec<Vec<Event>> {
    let size = match dice.below(4) {
        0 => 1,
        1 => 3,
        2 => 17,
        _ => events.len().max(1),
    };
    events.chunks(size).map(<[Event]>::to_vec).collect()
}

/// 照分叉要照的几样写子会话的 `session.created`：分叉点写的是 `upto`。
fn created(plan: &ForkPlan, upto: Seq, seal_after: Option<std::num::NonZeroU32>) -> SessionCreated {
    let mut created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    created.policy = plan.policy.clone();
    created.permission = plan.permission.clone();
    created.parent = Some(session_id());
    created.depth = Some(1);
    created.model = plan.model.clone();
    created.persona = plan.persona.clone();
    created.preset = plan.preset.clone();
    created.forked = Some(Forked {
        from: session_id(),
        upto,
        seal_after,
        within: plan.within,
    });
    created
}

/// 底子 `base`（到分叉点为止）新旧各写第一批事件、载入支线，比。`fed` 是分叉一块块喂好的底子：写完第一条接着交给载入。
fn fork_both(
    seed: u64,
    base: &[Event],
    plan: &ForkPlan,
    fed: BaseFeed,
    dice: &mut Rng,
    seen: &mut Seen,
) {
    let child = SessionId::parse(CHILD_SESSION).unwrap();
    let seal_after = std::num::NonZeroU32::new(u32::try_from(dice.below(3)).unwrap());
    let placeholder = plan.within.map(|_| "Forked; the parent keeps going.");
    if plan.within.is_none() {
        // fork_within 配普通分叉的、分叉点写错了的：照样同一个错。
        let wrong = created(
            plan,
            Seq::new(plan.upto.get().saturating_sub(1)).unwrap_or(seq(1)),
            seal_after,
        );
        let old = old::fork_on(
            child.clone(),
            id(900),
            person(),
            at(30),
            base,
            None,
            wrong.clone(),
            policy(),
            environment("~/src/miyu"),
        )
        .map(|(_, actions)| lines(&appended_events(&actions)));
        let new = Session::fork(
            child.clone(),
            &fed,
            id(900),
            person(),
            at(30),
            wrong,
            policy(),
            environment("~/src/miyu"),
        )
        .map(|events| lines(&events));
        assert_eq!(new, old, "种子 {seed}：分叉点写错了");
        seen.wrong_upto += usize::from(old.is_err());
    }
    let created = created(plan, plan.upto, seal_after);
    let old = old::fork_on(
        child.clone(),
        id(900),
        person(),
        at(30),
        base,
        placeholder,
        created.clone(),
        policy(),
        environment("~/src/miyu"),
    );
    let new = match placeholder {
        Some(placeholder) => Session::fork_within(
            child.clone(),
            &fed,
            placeholder,
            id(900),
            person(),
            at(30),
            created,
            policy(),
            environment("~/src/miyu"),
        ),
        None => Session::fork(
            child.clone(),
            &fed,
            id(900),
            person(),
            at(30),
            created,
            policy(),
            environment("~/src/miyu"),
        ),
    };
    let old_lines = old
        .as_ref()
        .map(|(_, actions)| lines(&appended_events(actions)));
    assert_eq!(
        new.as_ref().map(|events| lines(events)),
        old_lines.clone(),
        "种子 {seed}：写下的第一批逐字节一样"
    );
    let Ok((mut live, actions)) = old else {
        return;
    };
    // 支线自己接着走几步：清空（压缩起点挪到底子后面），再撤掉、恢复（起点挪回底子里）。
    let mut own = appended_events(&actions);
    let last = |own: &[Event]| own.last().map_or(0, |event| event.seq.get());
    live.handle(stored(last(&own)));
    if dice.below(2) == 0 {
        let events = appended_events(&live.handle(clear(901)));
        if let Some(started) = events
            .first()
            .filter(|event| matches!(event.body, Body::TurnStarted(_)))
        {
            seen.cleared += 1;
            let turn = started.seq.get();
            own.extend(events);
            live.handle(stored(last(&own)));
            if dice.below(2) == 0 {
                let actions = live.handle(revert(902, turn));
                let mut events = appended_events(&actions);
                // 撤掉压缩要先读回日志（施工 6-9）：照底子、自己的日志读回去。
                for action in &actions {
                    if let crate::session::Action::ReadBack { from, .. } = action {
                        let back = base
                            .iter()
                            .chain(&own)
                            .filter(|event| event.seq >= *from)
                            .cloned()
                            .collect();
                        let input = Input::ReadBack {
                            at: at(58),
                            from: *from,
                            events: back,
                        };
                        events.extend(appended_events(&live.handle(input)));
                    }
                }
                seen.reverted += usize::from(!events.is_empty());
                own.extend(events);
                live.handle(stored(last(&own)));
                if dice.below(2) == 0 {
                    own.extend(appended_events(&live.handle(unrevert(903))));
                }
            }
        }
    }
    let ledger_from = fed.ledger().compacted().map(Seq::next);
    if ledger_from.is_some_and(|from| from > seq(1)) {
        seen.compacted += 1;
    }
    // 载入支线：底子一块块喂的、分叉喂好交过来的（施工 V-39 下要走的那条），各和旧的比。
    let first = dice.below(own.len() as u64) as usize + 1;
    let (old_loaded, old_whole) = load_old(base, &own, first);
    let mut feed = BaseFeed::new(session_id());
    for block in blocks(base, dice) {
        feed.feed(block);
    }
    let (new_loaded, new_whole) = load_new(base, feed, &own, first);
    assert_eq!(new_loaded, old_loaded, "种子 {seed}：载入支线一样");
    let (handed_loaded, _) = load_new(base, fed, &own, first);
    assert_eq!(
        handed_loaded, old_loaded,
        "种子 {seed}：分叉喂好的底子交给载入，一样"
    );
    match (old_whole, new_whole) {
        (false, true) => seen.whole_new_only += 1,
        (true, true) => seen.whole_both += 1,
        (true, false) => panic!("种子 {seed}：旧的要整份、新的不要"),
        (false, false) => {}
    }
    seen.loaded += usize::from(!old_loaded.starts_with("Err"));
    // 父会话没写到分叉点（被删了一截、读到一半）：同一个错。
    if base.len() > 1 {
        let short = &base[..base.len() - 1];
        let old =
            old::loader(child.clone(), Some(short.to_vec()), &own, policy().reports).map(|_| ());
        let new = Session::loader(
            child,
            Some(BaseFeed::of(&session_id(), short)),
            &own,
            policy().reports,
        )
        .map(|_| ());
        assert_eq!(new, old, "种子 {seed}：底子没写到分叉点");
        seen.short += usize::from(old.is_err());
    }
}

/// 事件写成的行。
fn lines(events: &[Event]) -> Vec<String> {
    events.iter().map(Event::to_line).collect()
}

/// 旧的载入支线：头一块是自己日志的前 `first` 条，再一条条喂；对不上的照整份。交回结果的 `Debug`、照整份走了没有。
fn load_old(base: &[Event], own: &[Event], first: usize) -> (String, bool) {
    let child = SessionId::parse(CHILD_SESSION).unwrap();
    let loader = old::loader(
        child.clone(),
        Some(base.to_vec()),
        &own[..first],
        policy().reports,
    );
    finish(loader, own, first, |own| {
        old::load_forked(
            child.clone(),
            base,
            own,
            at(55),
            policy(),
            environment("~/src/miyu"),
        )
    })
}

/// 新的载入支线：底子是喂好的 `feed`，别的同 [`load_old`]。
fn load_new(base: &[Event], feed: BaseFeed, own: &[Event], first: usize) -> (String, bool) {
    let child = SessionId::parse(CHILD_SESSION).unwrap();
    let loader = Session::loader(child.clone(), Some(feed), &own[..first], policy().reports);
    finish(loader, own, first, |own| {
        Session::load_forked(
            child.clone(),
            base,
            own,
            at(55),
            policy(),
            environment("~/src/miyu"),
        )
    })
}

/// 喂完自己的日志、收尾；对不上的照 `whole` 整份走。
fn finish(
    loader: Result<crate::session::Loader, LoadError>,
    own: &[Event],
    first: usize,
    whole: impl FnOnce(Vec<Event>) -> Result<(Session, Vec<crate::session::Action>), LoadError>,
) -> (String, bool) {
    let mut loader = match loader {
        Ok(loader) => loader,
        Err(error) => return (format!("Err({error:?})"), false),
    };
    let fed = loader.feed(own[..first].to_vec()).and_then(|()| {
        own[first..]
            .iter()
            .try_for_each(|event| loader.feed(vec![event.clone()]))
    });
    if let Err(error) = fed {
        return (format!("Err({error:?})"), false);
    }
    if loader.needs_whole(&policy()) {
        return (format!("{:?}", whole(own.to_vec())), true);
    }
    (
        format!(
            "{:?}",
            loader.finish(at(55), policy(), environment("~/src/miyu"))
        ),
        false,
    )
}
