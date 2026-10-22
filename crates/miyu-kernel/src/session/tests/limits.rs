//! 给头看的限额（施工 6-3 补，`docs/blueprint/kernel/session.md` 的 `context_limits()`）：窗口照交来的，压缩线和内核判到线
//! 用的是同一条（`compaction.md` 第二条第 2 条）。
//!
//! 策略里压缩的数照出厂的：输出预留的上限 20000、余量 13000，压缩线至多窗口的 85%、余量至多窗口的 5%（施工 6-11 再补）。

use super::*;
use crate::estimate::Flat;
use crate::id::ModuleId;
use crate::session::{Breakdown, Compaction, ContextLimits, Said};

/// 出厂的四个数；尾巴、图片的数这里用不上。
const COMPACTION: Compaction = Compaction {
    reserve_cap: 20_000,
    margin: 13_000,
    line_percent: 85,
    margin_percent: 5,
    tail: 16_000,
    lead: 0,
    price: Flat {
        image: 2_000,
        file: 2_000,
    },
    rebuild: None,
    pause: None,
    shorten: None,
    isolate: false,
};

/// 策略里有压缩的会话。
fn compacting() -> Session {
    let mut policy = policy();
    policy.compaction = Some(COMPACTION);
    session_with(policy)
}

/// 交一份限额：替身的模型，窗口、最大输出照给的。
fn hand(session: &mut Session, window: Option<u64>, max_output: Option<u64>) {
    let actions = session.handle(Input::Limits(Limits {
        model: crate::testkit::model(),
        window,
        max_output,
        images: None,
        blind: false,
        identity: None,
    }));
    assert!(actions.is_empty(), "交限额什么动作都不出：{actions:?}");
}

fn limits(window: Option<u64>, compaction_line: Option<u64>) -> ContextLimits {
    ContextLimits {
        window,
        compaction_line,
    }
}

#[test]
fn before_any_limits_both_are_absent() {
    assert_eq!(compacting().context_limits(), limits(None, None));
}

#[test]
fn the_line_reserves_the_smaller_of_max_output_and_the_cap() {
    let mut session = compacting();
    hand(&mut session, Some(1_000_000), Some(393_216));
    assert_eq!(
        session.context_limits(),
        limits(Some(1_000_000), Some(850_000)),
        "大窗口：窗口的 85% 比减掉预留 20000、余量 13000 的 967000 小"
    );
    hand(&mut session, Some(128_000), Some(8_000));
    assert_eq!(
        session.context_limits(),
        limits(Some(128_000), Some(108_800)),
        "最大输出比上限小：预留 8000，余量是窗口的 5% 6400，减下来 113600，比窗口的 85% 大"
    );
    hand(&mut session, Some(60_000), None);
    assert_eq!(
        session.context_limits(),
        limits(Some(60_000), Some(37_000)),
        "没报最大输出：预留照上限，余量是窗口的 5% 3000；再交一次照新的"
    );
}

#[test]
fn too_small_a_window_has_no_line() {
    let mut session = compacting();
    hand(&mut session, Some(21_052), None);
    assert_eq!(
        session.context_limits(),
        limits(Some(21_052), None),
        "减掉预留 20000、余量 1052 正好是 0"
    );
    hand(&mut session, Some(21_053), None);
    assert_eq!(session.context_limits(), limits(Some(21_053), Some(1)));
    hand(&mut session, Some(16_000), Some(4_000));
    assert_eq!(
        session.context_limits(),
        limits(Some(16_000), Some(11_200)),
        "小窗口：预留 4000，余量 800"
    );
    hand(&mut session, Some(8_000), None);
    assert_eq!(
        session.context_limits(),
        limits(Some(8_000), None),
        "窗口比输出预留还小"
    );
}

#[test]
fn without_a_window_there_is_no_line_either() {
    let mut session = compacting();
    hand(&mut session, None, Some(8_000));
    assert_eq!(session.context_limits(), limits(None, None));
}

#[test]
fn without_compaction_in_the_policy_only_the_window() {
    let mut session = session();
    hand(&mut session, Some(1_000_000), Some(393_216));
    assert_eq!(session.context_limits(), limits(Some(1_000_000), None));
}

/// 这时的上下文用量（施工 8-15）：没交过限额的、策略里没有压缩的算不了；还没请求过的照本地估算；答完了照供应商报的那一次
/// 起算（锚盖住了整份请求和回复），加上后来进来的那一点。
#[test]
fn the_context_used_follows_the_compaction_estimate() {
    assert_eq!(compacting().context_used(), None, "没交过限额");
    let mut plain = session();
    hand(&mut plain, Some(1_000_000), None);
    assert_eq!(plain.context_used(), None, "策略里没有压缩");
    let mut session = compacting();
    hand(&mut session, Some(1_000_000), None);
    let empty = session.context_used().expect("交了限额就算得出");
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    session.handle(hooks_done(turn3(), Vec::new()));
    let asked = session.context_used().expect("算得出");
    assert!(asked > empty, "多了人说的一句：{asked} > {empty}");
    let reply = super::executor::answer(&mut session, 5, "好");
    assert!(!reply.is_empty());
    let reported = super::executor::usage();
    let total = reported.uncached + reported.cache_read + reported.cache_write + reported.output;
    // 锚是供应商报的这一次，锚以后新进有效历史的（这一轮结束时记的几样）照本地估算加上去。
    let used = session.context_used().expect("算得出");
    assert!(
        (total..total + 50).contains(&used),
        "照供应商报的那一次起算：{used}，报的 {total}"
    );
}

/// 模块 `module` 注入的一块，类别是 `kind`。
fn fact(module: &str, kind: &str, text: &str) -> Injection {
    let mut injected = super::executor::injection(module, text);
    injected.fact.kind = crate::id::FactKind::parse(kind).unwrap();
    injected
}

/// 这时的上下文一块块的（施工 CX-1 上，`protocol.md` 的 `session.context`）：system、每件工具、示范对话、检查点摘要、
/// 对话里每条照是谁的、事实照模块和类别归，各自本地估；用量同 `context_used`，答过以后有锚（供应商报的那一次）。
#[test]
fn the_breakdown_splits_the_context_by_who_put_it_there() {
    let mut session = compacting();
    let before = session.breakdown();
    assert_eq!(
        (before.used, before.anchor),
        (None, None),
        "没交过限额：算不了用量"
    );
    hand(&mut session, Some(1_000_000), None);
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    session.handle(hooks_done(
        turn3(),
        vec![
            fact("memory", "memory", "常驻的几条记忆"),
            fact("memory", "recall", "想起来的一段"),
        ],
    ));
    // 内核自己的两条事实加这两条：落了盘，请求是第 7 条。
    session.handle(stored(7));
    let asked = session.breakdown();
    assert_eq!(asked.used, session.context_used());
    assert_eq!(asked.anchor, None, "还没答过：全是估的");
    assert_eq!(asked.limits, session.context_limits());
    let of = |breakdown: &Breakdown, said: &Said| {
        breakdown
            .said
            .iter()
            .find(|(which, _)| which == said)
            .map(|(_, tokens)| *tokens)
    };
    let memory = |kind: &str| Said::Fact {
        module: Some(ModuleId::parse("memory").unwrap()),
        kind: crate::id::FactKind::parse(kind).unwrap(),
    };
    assert!(
        of(&asked, &Said::User).is_some_and(|tokens| tokens > 0),
        "{:?}",
        asked.said
    );
    assert_eq!(
        of(&asked, &memory("memory")),
        Some(crate::estimate::text("常驻的几条记忆"))
    );
    assert_eq!(
        of(&asked, &memory("recall")),
        Some(crate::estimate::text("想起来的一段"))
    );
    assert_eq!(of(&asked, &Said::Assistant), None, "还没回复");
    let request = session.policy.assembler.assemble(&session.history);
    assert_eq!(asked.system, crate::estimate::text(&request.system));
    assert_eq!(asked.tools.len(), request.tools.len());
    super::executor::call_tools(&mut session, 7, &[("read", "{}")]);
    super::executor::allowing(&mut session, stored(9));
    session.handle(super::executor::done(
        super::executor::call(8, 1),
        "读到的一页",
    ));
    let answered = session.breakdown();
    let reported = super::executor::usage();
    let total = reported.uncached + reported.cache_read + reported.cache_write + reported.output;
    assert_eq!(
        answered.anchor,
        Some(total),
        "答过了：用量照供应商报的那一次起算"
    );
    assert!(of(&answered, &Said::Assistant).is_some_and(|tokens| tokens > 0));
    assert!(
        of(&answered, &Said::ToolResult).is_some_and(|tokens| tokens > 0),
        "{:?}",
        answered.said
    );
    let parts = answered.system
        + answered.tools.iter().map(|(_, tokens)| tokens).sum::<u64>()
        + answered.demos
        + answered.summary
        + answered.said.iter().map(|(_, tokens)| tokens).sum::<u64>();
    assert_eq!(answered.estimated_total(), parts);
    session.handle(Input::Limits(Limits {
        model: crate::origin::Model {
            endpoint: crate::id::ProviderId::parse("other").unwrap(),
            model: crate::id::ModelName::parse("other-1").unwrap(),
        },
        window: Some(1_000_000),
        max_output: None,
        images: None,
        blind: false,
        identity: None,
    }));
    assert_eq!(
        session.breakdown().anchor,
        None,
        "换了模型：锚是别的模型报的，不算"
    );
}
