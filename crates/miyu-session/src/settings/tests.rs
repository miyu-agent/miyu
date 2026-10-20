//! `compaction.prepare`：默认开，系统配置、个人设置两层能写，项目配置不能，下一轮生效。

use super::*;
use miyu_config::{Applies, Layer, Values};

#[test]
fn preparing_is_on_by_default_and_applies_next_turn() {
    let defaults = CompactionSettings::from(&Values::defaults(CompactionSettings::ITEMS));
    assert!(defaults.prepare);
    let item = &CompactionSettings::ITEMS[0];
    assert_eq!(item.key, "compaction.prepare");
    assert_eq!(item.layers, &[Layer::System, Layer::Personal]);
    assert_eq!(item.applies, Applies::NextTurn);
}

/// `jobs.max_running`（施工 7-12 下，照 Claude Code）：默认 30 分钟，最多 2 小时，下一轮生效。
#[test]
fn background_commands_run_half_an_hour_by_default() {
    let defaults = JobSettings::from(&Values::defaults(JobSettings::ITEMS));
    assert_eq!(defaults.max_running, Duration::from_secs(30 * 60));
    let item = &JobSettings::ITEMS[0];
    assert_eq!(item.key, "jobs.max_running");
    assert_eq!(item.layers, &[Layer::System, Layer::Personal]);
    assert_eq!(item.applies, Applies::NextTurn);
    assert_eq!(
        item.kind,
        miyu_config::Kind::Duration { min: 60, max: 7200 }
    );
}
