//! 压缩触发线可配（施工 CT-1，`docs/blueprint/compaction.md` 第二条第 2 条）：系统配置 `compaction.line` 改了，没钉的会话下一个
//! 回合照新的压；会话钉的（`Command::SetCompactionLine`）压过配置，下一个回合生效，去掉的回到跟着配置。换了的那一轮记一条换快照的
//! `session.policy_changed`，给头看的压缩线跟着换。
//!
//! 窗口 20 万、没报最大输出（减法那一条是 17 万），关掉提前压（到线当场压）。第一句 13 万 token，比尾巴的预算大，到线时有得压：
//! 照出厂的 85（17 万）不压，60（12 万）、50（10 万）压。剧本报的用量和它差不多：换了快照的那一轮锚作废（`compaction.md`
//! 第一条第 1 款，锚不跨换快照），照整份本地估算，所以这里用真的长度，不靠剧本报的数。

use std::sync::Arc;

use tokio::sync::watch;

use miyu_config::merge::Resolved;
use miyu_config::secret::{Reference, Secret};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::session::Command;
use miyu_session::ConfigSource;
use miyu_session::testkit::{Play, Script};

use crate::support::routing::resolved;
use crate::support::{Home, ask, say, stop, until_turn_ends, watch as watching};

/// 不变的一份配置。
#[derive(Debug)]
struct Fixed(Resolved);

impl ConfigSource for Fixed {
    fn with_project(&self, _: &str) -> Resolved {
        self.0.clone()
    }

    fn secret(&self, _: &Reference) -> Option<Secret> {
        None
    }
}

/// 关掉提前压，再接上 `more` 的一份配置。
fn source(more: &str) -> Arc<dyn ConfigSource> {
    Arc::new(Fixed(resolved(&format!(
        "[compaction]\nprepare = false\n{more}"
    ))))
}

/// 日志里压缩的摘要，照先后。
fn summaries(log: &[Event]) -> Vec<String> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => Some(compacted.summary.clone()),
            _ => None,
        })
        .collect()
}

/// 日志里换了快照的那几条有几条。
fn swaps(log: &[Event]) -> usize {
    log.iter()
        .filter(
            |event| matches!(&event.body, Body::PolicyChanged(changed) if changed.policy.is_some()),
        )
        .count()
}

/// 第一句：52 万个字节，约 13 万 token。
static LONG: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| "a".repeat(520_000));

/// 剧本：答 `plays`，窗口 20 万，每次报 13.1 万。
fn script(plays: impl IntoIterator<Item = Play>) -> Script {
    Script::new(plays).window(200_000).reports(131_000)
}

#[tokio::test]
async fn a_changed_config_line_takes_the_next_turn() {
    let mut home = Home::new();
    let (config, configs) = watch::channel(source(""));
    home.configs = configs;
    let script = script([
        Play::Says("好。"),
        Play::Says("嗯。"),
        Play::Says("<summary>照 60 压的摘要</summary>"),
        Play::Says("行。"),
    ]);
    let handle = home.create(&script).await;
    let mut pushes = watching(&handle).await;
    for (id, words) in [("cmd-1", LONG.as_str()), ("cmd-2", "再说一句")] {
        ask(&handle, id, say(words)).await.expect("会话在跑");
        until_turn_ends(&mut pushes).await;
    }
    assert_eq!(handle.limits().compaction_line, Some(170_000));
    assert!(summaries(&home.log(handle.id())).is_empty(), "照 85 不压");
    config.send_replace(source("line = 60\n"));
    ask(&handle, "cmd-3", say("第三句"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert_eq!(handle.limits().compaction_line, Some(120_000), "下一轮换上");
    stop(&handle).await;
    let log = home.log(handle.id());
    assert_eq!(summaries(&log), ["照 60 压的摘要"]);
    assert_eq!(swaps(&log), 1, "只在换的那一轮记一条换快照");
}

#[tokio::test]
async fn a_pinned_line_beats_the_config_and_null_follows_it_again() {
    let mut home = Home::new();
    home.configs = watch::channel(source("")).1;
    let script = script([
        Play::Says("好。"),
        Play::Says("<summary>照 50 压的摘要</summary>"),
        Play::Says("嗯。"),
        Play::Says("行。"),
    ]);
    let handle = home.create(&script).await;
    let mut pushes = watching(&handle).await;
    ask(&handle, "cmd-1", say(&LONG)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let pin = |percent| Command::SetCompactionLine { percent };
    ask(&handle, "pin-1", pin(Some(50)))
        .await
        .expect("会话在跑");
    assert_eq!(
        handle.limits().compaction_line,
        Some(170_000),
        "钉了没开回合的还是原来的"
    );
    ask(&handle, "cmd-2", say("再说一句"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert_eq!(handle.limits().compaction_line, Some(100_000));
    stop(&handle).await;
    // 重启以后照日志里记的快照、钉的数。
    let handle = home.load(handle.id(), &script).await;
    let mut pushes = watching(&handle).await;
    assert_eq!(handle.limits().compaction_line, Some(100_000), "载入照日志");
    ask(&handle, "pin-2", pin(None)).await.expect("会话在跑");
    ask(&handle, "cmd-3", say("第三句"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert_eq!(
        handle.limits().compaction_line,
        Some(170_000),
        "去掉了回到跟着配置"
    );
    stop(&handle).await;
    let log = home.log(handle.id());
    assert_eq!(summaries(&log), ["照 50 压的摘要"]);
    assert_eq!(swaps(&log), 2, "钉上、去掉各换一次");
}
