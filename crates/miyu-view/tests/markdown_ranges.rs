//! 导出的范围（施工 SE-1 中下中，`range`）：`context` 照内核的有效历史算，和请求组装看到的是同一份：压缩过的从检查点起，
//! `agent` 把摘要放最前、`read`、`chat` 写一句省掉了几轮、压缩那一条照时间线留着；压缩被撤掉的回到更早的；`{"turns": N}` 最近
//! N 轮，`read`、`chat` 写省掉了几轮，`agent` 的开头写一行英文；撤掉的照旧不进。全照真会话跑。

use std::sync::Arc;

use miyu_kernel::testkit::{Line, Stage};
use miyu_view::Projector;
use miyu_view::markdown::{Options, Range, Sources};

use crate::markdown_presets::rendered;
use crate::support::*;

/// 照真会话的日志喂投影、记原文；要上下文的照有效历史算好。
fn fed(stage: &Stage, context: bool) -> (Vec<miyu_view::Entry>, Sources) {
    let mut projector = Projector::new(Arc::new(texts("zh")), None);
    let mut sources = match context {
        true => Sources::keeping_history(),
        false => Sources::default(),
    };
    for event in stage.log() {
        projector.event(event);
        sources.see(event);
    }
    sources.settle();
    (projector.entries().to_vec(), sources)
}

fn export(stage: &Stage, options: Options) -> String {
    let (entries, sources) = fed(stage, options.range == Range::Context);
    rendered(&entries, &sources, options).text
}

/// 说 first，压一次，说 second。
fn compacted() -> Stage {
    let mut stage = stage();
    stage.limits(Some(200_000), Some(8_000));
    stage.model([Line::says("one"), Line::says("two")]);
    stage.say("first");
    summarizes(&mut stage, "summary of the talk");
    stage.request_compaction(None);
    stage.say("second");
    stage
}

#[test]
fn agent_context_starts_from_the_checkpoint_with_the_summary_first() {
    let stage = compacted();
    let got = export(&stage, Options::AGENT);
    assert!(
        got.contains(
            "- Earlier parts of the session were compacted; the summary below stands in for them.\n\n## Summary\n\nsummary of the talk\n\n## User\n\nsecond\n\n## Assistant\n\ntwo\n"
        ),
        "{got}"
    );
    for gone in ["first", "one", "上下文压缩了"] {
        assert!(
            !got.contains(gone),
            "检查点以前的、压缩那一条不进：{gone}\n{got}"
        );
    }
    let all = export(
        &stage,
        Options {
            range: Range::All,
            ..Options::AGENT
        },
    );
    assert!(
        all.contains("first") && all.contains("> 上下文压缩了"),
        "写了 range: all 的全要：{all}"
    );
}

#[test]
fn read_and_chat_context_say_how_many_turns_are_left_out() {
    let stage = compacted();
    for options in [Options::READ, Options::CHAT] {
        let got = export(
            &stage,
            Options {
                range: Range::Context,
                ..options
            },
        );
        assert!(got.contains("\n\n> 前面省掉了 1 轮\n\n"), "{got}");
        assert!(!got.contains("first"), "{got}");
        let notice = got.find("> 上下文压缩了").expect("压缩那一条照时间线留着");
        assert!(notice < got.find("second").expect("有"), "{got}");
        assert!(
            got.contains("<summary>摘要</summary>\n\nsummary of the talk"),
            "摘要折在下面：{got}"
        );
    }
}

#[test]
fn an_undone_compaction_puts_the_earlier_turns_back_in_context() {
    let mut stage = stage();
    stage.limits(Some(200_000), Some(8_000));
    stage.model([Line::says("one"), Line::says("two"), Line::says("three")]);
    stage.say("first");
    summarizes(&mut stage, "summary of the talk");
    stage.request_compaction(None);
    let compaction = stage.turns()[1];
    stage.say("second");
    stage.revert(compaction);
    stage.say("third");
    let got = export(&stage, Options::AGENT);
    assert!(
        got.contains("## User\n\nfirst") && got.contains("## User\n\nthird"),
        "撤掉了压缩，上下文回到第一轮：{got}"
    );
    for gone in ["second", "summary of the talk", "Earlier parts"] {
        assert!(!got.contains(gone), "{gone}：{got}");
    }
}

#[test]
fn the_last_turns_are_cut_by_turn_and_undone_ones_stay_out() {
    let mut stage = stage();
    stage.model([
        Line::says("one"),
        Line::says("two"),
        Line::says("three"),
        Line::says("four"),
    ]);
    stage.say("first");
    stage.say("second");
    stage.say("third");
    stage.say("undone");
    let last = stage.turns()[3];
    stage.revert(last);
    let read = export(
        &stage,
        Options {
            range: Range::Turns(2),
            ..Options::READ
        },
    );
    assert!(
        read.contains(
            "\n\n> 前面省掉了 1 轮\n\n## User\n\nsecond\n\n## Assistant\n\ntwo\n\n## User\n\nthird"
        ),
        "撤掉的不算一轮：{read}"
    );
    assert!(
        !read.contains("first") && !read.contains("undone"),
        "{read}"
    );
    let agent = export(
        &stage,
        Options {
            range: Range::Turns(1),
            ..Options::AGENT
        },
    );
    assert!(
        agent.contains("- Only the latest turns are included; 2 earlier turns are left out."),
        "{agent}"
    );
    assert!(
        agent.contains("third") && !agent.contains("second"),
        "{agent}"
    );
    let more = export(
        &stage,
        Options {
            range: Range::Turns(9),
            ..Options::CHAT
        },
    );
    assert!(
        more.contains("first") && !more.contains("省掉"),
        "比有的多的照全部：{more}"
    );
}
