//! `compaction.prepare`：默认开，配置能写，项目配置不能，下一轮生效。`compaction.line`（施工 CT-1）同样只在系统层。

use super::*;
use miyu_config::{Applies, Layer, Values};

#[test]
fn preparing_is_on_by_default_and_applies_next_turn() {
    let defaults = CompactionSettings::from(&Values::defaults(CompactionSettings::ITEMS));
    assert!(defaults.prepare);
    let item = &CompactionSettings::ITEMS[1];
    assert_eq!(item.key, "compaction.prepare");
    assert_eq!(item.layers, &[Layer::System]);
    assert_eq!(item.applies, Applies::NextTurn);
}

/// `compaction.line`（施工 CT-1，2026-11-12 项目主人定）：出厂 85，20 到 95 的整数，19、96 超范围；只在系统层，下一轮生效；
/// 会话钉的压过配置的；协议查的范围就是声明的这一段。
#[test]
fn the_compaction_line_is_85_percent_between_20_and_95() {
    let defaults = CompactionSettings::from(&Values::defaults(CompactionSettings::ITEMS));
    assert_eq!(defaults.line, 85);
    let item = &CompactionSettings::ITEMS[0];
    assert_eq!(item.key, "compaction.line");
    assert_eq!(item.layers, &[Layer::System]);
    assert_eq!(item.applies, Applies::NextTurn);
    assert_eq!(item.kind, miyu_config::Kind::Int { min: 20, max: 95 });
    for (percent, fits) in [(19, false), (20, true), (95, true), (96, false)] {
        assert_eq!(
            item.kind.check(&miyu_config::Value::Int(percent)).is_ok(),
            fits,
            "{percent}"
        );
    }
    assert_eq!(CompactionSettings::line_range(), 20..=95);
    assert_eq!(defaults.line_for(None), 85, "没钉的照配置");
    assert_eq!(defaults.line_for(Some(50)), 50, "钉的压过配置");
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
/// 只有系统一层，下一轮生效，设置页「隐私与安全」那一页「受保护的命令」一组的开关。关掉的那几类照类的写法交出去。最后一项是
/// 确认等多久（施工 CD-1），另有一条测试。
#[test]
fn each_protected_group_has_a_switch_that_is_on_by_default() {
    let switches = &ConfirmSettings::ITEMS[..Group::ALL.len()];
    let keys: Vec<&str> = switches.iter().map(|item| item.key).collect();
    let expected: Vec<String> = Group::ALL
        .iter()
        .map(|group| format!("permission.confirm.{}", group.code()))
        .collect();
    assert_eq!(keys, expected);
    for item in switches {
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
        timeout: 300,
    };
    let all: Vec<&str> = Group::ALL.iter().map(|group| group.code()).collect();
    assert_eq!(none.waived(), all);
}

/// 等人确认等多久（施工 CD-1，2026-11-12 项目主人定）：`permission.confirm.timeout`，秒，出厂 300（5 分钟），0 到 86400 的整数，
/// 0 是一直等；只在系统层，下一轮生效，设置页「隐私与安全」那一页「权限」一组的数字框。交给内核的是毫秒。
#[test]
fn confirmations_wait_five_minutes_by_default_and_zero_waits_forever() {
    let item = ConfirmSettings::ITEMS
        .iter()
        .find(|item| item.key == "permission.confirm.timeout")
        .expect("声明了这一项");
    assert_eq!(item.default, Some(miyu_config::Value::Int(300)));
    assert_eq!(item.kind, miyu_config::Kind::Int { min: 0, max: 86400 });
    assert_eq!(item.layers, &[Layer::System]);
    assert_eq!(item.applies, Applies::NextTurn);
    assert_eq!((item.ui.page, item.ui.group), ("privacy", "permission"));
    assert_eq!(item.ui.control, miyu_config::Control::Number);
    let defaults = ConfirmSettings::from(&Values::defaults(ConfirmSettings::ITEMS));
    assert_eq!(defaults.wait_ms(), Some(300_000));
    let forever = ConfirmSettings {
        timeout: 0,
        ..defaults.clone()
    };
    assert_eq!(forever.wait_ms(), None, "0 是一直等");
    let longest = ConfirmSettings {
        timeout: 86400,
        ..defaults
    };
    assert_eq!(longest.wait_ms(), Some(86_400_000));
}
