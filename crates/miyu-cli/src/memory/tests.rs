//! 一条记忆印成一行（`docs/blueprint/cli/memory.md`「样子」）：编号照最长的对齐、日期照时区换天、作废的带原因。

use serde_json::json;

use miyu_kernel::time::UtcOffset;

use super::shown::{cards, listed};
use crate::diagnostic::Problem;
use crate::language::{Language, Organized};

fn memory(id: &str, at: &str, text: &str, retired: Option<&str>) -> serde_json::Value {
    json!({"id": id, "at": at, "text": text, "retired": retired, "by": "person", "class": "user", "sources": []})
}

#[test]
fn one_line_each_ids_aligned_and_forgotten_ones_marked() {
    let memories = [
        memory("m12", "2026-10-08T03:00:00.000Z", "用户住在上海", None),
        memory("m8", "2026-10-07T03:00:00.000Z", "回答要短，先说结论", None),
        memory(
            "m3",
            "2026-10-07T02:00:00.000Z",
            "用户养了两只猫",
            Some("试一下"),
        ),
        memory("m2", "2026-10-07T01:00:00.000Z", "用户用 A 卡", Some("")),
    ];
    assert_eq!(
        listed(&memories, UtcOffset::UTC, &Language::Chinese),
        [
            "m12  2026-10-08  用户住在上海",
            "m8   2026-10-07  回答要短，先说结论",
            "m3   2026-10-07  用户养了两只猫（已作废：试一下）",
            "m2   2026-10-07  用户用 A 卡（已作废）",
        ]
    );
    assert_eq!(
        listed(&memories[2..3], UtcOffset::UTC, &Language::English),
        ["m3  2026-10-07  用户养了两只猫 (forgotten: 试一下)"]
    );
}

/// 认识的人那一类正文前面写是谁（施工 E-1 上）。
#[test]
fn a_person_shows_who() {
    let mut person = memory("m4", "2026-10-21T03:00:00.000Z", "小林用 RX 7800 XT", None);
    person["class"] = json!("person");
    person["who"] = json!("qq:2");
    assert_eq!(
        listed(&[person], UtcOffset::UTC, &Language::Chinese),
        ["m4  2026-10-21  [qq:2] 小林用 RX 7800 XT"]
    );
}

/// 发生过的事正文前面写涉及谁（施工 E-1 修四）；空列表的照旧。
#[test]
fn an_episode_shows_who_it_involves() {
    let mut episode = memory("m5", "2026-10-21T03:00:00.000Z", "小林把她 P 成了猪", None);
    episode["class"] = json!("episode");
    episode["people"] = json!(["qq:2", "小雨"]);
    let mut alone = memory("m6", "2026-10-21T03:00:00.000Z", "群里停电了", None);
    alone["people"] = json!([]);
    assert_eq!(
        listed(&[episode, alone], UtcOffset::UTC, &Language::Chinese),
        [
            "m5  2026-10-21  [qq:2, 小雨] 小林把她 P 成了猪",
            "m6  2026-10-21  群里停电了"
        ]
    );
}

/// 人卡一张一行（施工 E-1 下）：是谁后面括号里接叫法，照最长的对齐；作废的带原因。
#[test]
fn a_card_shows_who_and_names() {
    let shown = cards(
        &[
            json!({"who": "qq:2", "names": ["小林", "林林"], "text": "群友，喜欢拍照。", "at": "2026-10-22T03:00:00.000Z", "retired": null}),
            json!({"who": "老王", "names": [], "text": "大学室友。", "at": "2026-10-21T03:00:00.000Z", "retired": "认错了"}),
        ],
        UtcOffset::UTC,
        &Language::Chinese,
    );
    assert_eq!(shown[0], "qq:2 (小林, 林林)  2026-10-22  群友，喜欢拍照。");
    assert!(
        shown[1].starts_with(&format!("老王{}  2026-10-21  大学室友。", " ".repeat(11))),
        "{}",
        shown[1]
    );
    assert!(shown[1].contains("认错了"), "{}", shown[1]);
}

#[test]
fn the_day_follows_the_time_zone() {
    let late = [memory("m1", "2026-10-07T20:30:00.000Z", "晚上记的", None)];
    let tokyo = UtcOffset::from_minutes(9 * 60).expect("合范围");
    let new_york = UtcOffset::from_minutes(-4 * 60).expect("合范围");
    assert_eq!(
        listed(&late, tokyo, &Language::Chinese),
        ["m1  2026-10-08  晚上记的"]
    );
    assert_eq!(
        listed(&late, new_york, &Language::Chinese),
        ["m1  2026-10-07  晚上记的"]
    );
}

#[test]
fn nothing_says_so() {
    assert_eq!(Language::Chinese.no_memories(), "没有记忆");
    assert_eq!(Language::Chinese.nothing_found(), "没有找到结果");
    assert_eq!(Language::English.no_memories(), "no memories");
    assert_eq!(Language::Chinese.remembered("m4"), "已添加 m4");
    assert_eq!(Language::Chinese.changed("m5"), "已修改，新编号 m5");
    assert_eq!(Language::Chinese.cleared(3), "已清除 3 条记忆");
    assert_eq!(Language::English.cleared(3), "cleared 3 memories");
    assert_eq!(Language::English.cleared(1), "cleared 1 memory");
    // 同 `/dream` 的回执（施工 R-22 三补）：是 0 的不写、都是 0 的说没有要改的，末尾写用时。
    let organized = |given, revised, retired, summary, took_ms| Organized {
        given,
        revised,
        retired,
        summary,
        took_ms,
    };
    assert_eq!(
        Language::Chinese.dreamed(&organized(0, 0, 0, false, 2_000)),
        "无需整理，用时 2s"
    );
    assert_eq!(
        Language::Chinese.dreamed(&organized(4, 1, 2, false, 65_000)),
        "已整理：检查 4 条，修改 1 条，作废 2 条，用时 1m 05s"
    );
    assert_eq!(
        Language::Chinese.dreamed(&organized(9, 0, 0, true, 12_000)),
        "已整理：检查 9 条，没有要改的，摘要已更新，用时 12s"
    );
    assert_eq!(
        Language::English.dreamed(&organized(4, 0, 2, true, 12_000)),
        "organized memories: 4 checked, 2 retired, summary updated, took 12s"
    );
    assert_eq!(
        Language::English.dreamed(&organized(0, 0, 0, true, 1_000)),
        "nothing to organize, took 1s"
    );
    assert_eq!(
        Language::Chinese.dream_stopped(&organized(4, 0, 1, true, 400)),
        "已中断整理：检查 4 条，作废 1 条，用时 1s，其余下次继续",
        "叫停的不说摘要"
    );
}

#[test]
fn a_failed_dream_says_why() {
    // 施工 R-7 修（验收手册待定 10）：整理没成的，原话后面接上核心给的为什么（同 `miyu kb` 挂不上的目录）；太长的照旧说
    // 几个字；没给为什么的照原话。
    let failed = json!({
        "message": "整理记忆失败。",
        "data": {"reason": "dream_failed", "message": "model request failed: HTTP 401"},
    });
    assert_eq!(
        super::refused(&failed, &Language::Chinese),
        Problem::new("整理记忆失败：model request failed: HTTP 401")
    );
    let failed = json!({
        "message": "Memory organization failed.",
        "data": {"reason": "dream_failed", "message": "reply was not JSON"},
    });
    assert_eq!(
        super::refused(&failed, &Language::English),
        Problem::new("memory organization failed: reply was not JSON")
    );
    let plain =
        json!({"message": "记忆不可用。", "data": {"reason": "memory_unavailable", "why": "off"}});
    assert_eq!(
        super::refused(&plain, &Language::Chinese),
        Problem::new("记忆不可用")
    );
}
