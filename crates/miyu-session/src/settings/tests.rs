//! `compaction.prepare`：默认开，配置能写，项目配置不能，下一轮生效。

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

/// 受保护的命令（施工 HZ-1，设计 11 的 A18）：一类一项 `permission.confirm.<类>`，照 [`Group::ALL`] 的先后；出厂都开（照旧问），
/// 只有系统一层，下一轮生效，设置页「隐私与安全」那一页「受保护的命令」一组的开关。关掉的那几类照类的写法交出去。
#[test]
fn each_protected_group_has_a_switch_that_is_on_by_default() {
    let keys: Vec<&str> = ConfirmSettings::ITEMS.iter().map(|item| item.key).collect();
    let expected: Vec<String> = Group::ALL
        .iter()
        .map(|group| format!("permission.confirm.{}", group.code()))
        .collect();
    assert_eq!(keys, expected);
    for item in ConfirmSettings::ITEMS {
        assert_eq!(item.default, Some(miyu_config::Value::Bool(true)));
        assert_eq!(item.layers, &[Layer::System], "{}", item.key);
        assert_eq!(item.applies, Applies::NextTurn, "{}", item.key);
        assert_eq!((item.ui.page, item.ui.group), ("privacy", "protected"));
        assert_eq!(item.ui.control, miyu_config::Control::Toggle);
    }
    let defaults = ConfirmSettings::from(&Values::defaults(ConfirmSettings::ITEMS));
    assert!(defaults.waived().is_empty(), "出厂都问");
    let some = ConfirmSettings {
        git: false,
        container: false,
        ..defaults.clone()
    };
    assert_eq!(some.waived(), ["git", "container"]);
    let none = ConfirmSettings {
        git: false,
        delete: false,
        power: false,
        pipe_to_shell: false,
        container: false,
    };
    let all: Vec<&str> = Group::ALL.iter().map(|group| group.code()).collect();
    assert_eq!(none.waived(), all);
}
