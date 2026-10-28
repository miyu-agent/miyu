//! `compaction.prepare`：默认开，系统配置、个人设置两层能写，项目配置不能，下一轮生效。

use super::*;
use miyu_config::{Applies, Layer, Values};

#[test]
fn preparing_is_on_by_default_and_applies_next_turn() {
    let defaults = CompactionSettings::from(&Values::defaults(CompactionSettings::ITEMS));
    assert!(defaults.prepare);
    let item = &CompactionSettings::ITEMS[0];
    assert_eq!(item.key, "compaction.prepare");
    assert_eq!(item.layers, &[Layer::System]);
    assert_eq!(item.applies, Applies::NextTurn);
}

/// `jobs.max_running`（施工 7-12 下，照 Claude Code）：默认 30 分钟，最多 2 小时，下一轮生效。
#[test]
fn background_commands_run_half_an_hour_by_default() {
    let defaults = JobSettings::from(&Values::defaults(JobSettings::ITEMS));
    assert_eq!(defaults.max_running, Duration::from_secs(30 * 60));
    let item = &JobSettings::ITEMS[0];
    assert_eq!(item.key, "jobs.max_running");
    assert_eq!(item.layers, &[Layer::System]);
    assert_eq!(item.applies, Applies::NextTurn);
    assert_eq!(
        item.kind,
        miyu_config::Kind::Duration { min: 60, max: 7200 }
    );
}

/// `memory.extract_max_turns`、`memory.extract_max_wait`（施工 E-5）：一直不闲的会话攒 20 轮、或者最早那一轮等满一小时就
/// 不等闲抽；轮数 2 到 500，时长 10 分钟到一天；系统、个人两层，下一轮生效。
#[test]
fn a_busy_session_is_extracted_after_twenty_turns_or_an_hour() {
    let defaults = MemorySettings::from(&Values::defaults(MemorySettings::ITEMS));
    assert_eq!(defaults.extract_max_turns, 20);
    assert_eq!(defaults.extract_max_wait, Duration::from_secs(60 * 60));
    let item = |key: &str| {
        MemorySettings::ITEMS
            .iter()
            .find(|item| item.key == key)
            .unwrap_or_else(|| panic!("{key} 登记了"))
    };
    let turns = item("memory.extract_max_turns");
    assert_eq!(turns.kind, miyu_config::Kind::Int { min: 2, max: 500 });
    let wait = item("memory.extract_max_wait");
    assert_eq!(
        wait.kind,
        miyu_config::Kind::Duration {
            min: 600,
            max: 86_400
        }
    );
    for item in [turns, wait] {
        assert_eq!(item.layers, &[Layer::System]);
        assert_eq!(item.applies, Applies::NextTurn);
    }
}
