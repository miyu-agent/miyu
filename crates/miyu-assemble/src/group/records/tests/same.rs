//! 挑行、渲染分两半以后（施工 V-39 上）判官看到的字一个字节不变：照种子造三千段群聊（分六份并着跑），每一条都当要判的那一条，`count`、
//! `grow` 各取几样，新的 `records_in` 和以前的（`old.rs`）比；端点那样只交最后 [`Window::most`] 条的也比。
//!
//! 平时每份只比前 [`QUICK`] 段（施工 CI-2：三千段都比 Windows 的 debug 要近两分钟）；三千段都比的是名字以 `_for_longer`
//! 结尾的那六个（`#[ignore]`，CI 的 randomized 那一项跑）。

use std::collections::BTreeMap;

use miyu_kernel::event::Event;
use miyu_kernel::history::History;
use miyu_kernel::id::{ExternalId, Seq};

use super::old;
use crate::group::records::{Picked, Window, on_record, quoted_as, records_from, records_in};
use crate::test_support::{forward_texts, group_texts};

/// 照种子走的随机数（xorshift，同 V-37 下的对照测试）。
struct Dice(u64);

impl Dice {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n.max(1)
    }

    fn odds(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
}

const MEMBERS: [&str; 3] = [
    r#"{"kind":"external","venue":"qq:group:1","id":"qq:20017","role":"member"}"#,
    r#"{"kind":"external","venue":"qq:group:1","id":"qq:20018","role":"manager"}"#,
    r#"{"kind":"person","via":"qq:10001"}"#,
];
const BRIDGE: &str = r#"{"kind":"module","id":"onebot"}"#;
const WORDS: [&str; 5] = ["早", "", "@Miyu 在吗", "a\nb", "今天食堂的饭有点咸"];

/// 一段群聊：一行一条，序号从 1 数。编号只从十几个里挑，好撞上重复的、引用的、撤回的。
fn chat(dice: &mut Dice) -> Vec<Event> {
    let n = 5 + dice.below(80);
    let mut lines = Vec::new();
    for seq in 1..=n {
        let id = |dice: &mut Dice| format!("m{}", dice.below(14));
        let (kind, by, body) = match dice.below(10) {
            0..=5 => {
                let mut venue = format!(r#""msg":"{}","name":"群友{}""#, id(dice), dice.below(3));
                for (flag, odds) in [
                    ("ambient", 2),
                    ("asleep", 6),
                    ("backfill", 7),
                    ("show_ids", 3),
                    ("mentions_me", 5),
                ] {
                    if dice.odds(odds) {
                        venue.push_str(&format!(r#","{flag}":true"#));
                    }
                }
                if dice.odds(3) {
                    venue.push_str(&format!(r#","reply_to":"{}""#, id(dice)));
                }
                if dice.odds(5) {
                    venue.push_str(r#","media":[{"kind":"image","id":"i1"}]"#);
                }
                if dice.odds(8) {
                    venue.push_str(r#","forwards":[{"nodes":[{"name":"甲","text":"转来的"}]}]"#);
                }
                let words = WORDS[dice.below(5) as usize];
                let body = format!(
                    r#"{{"blocks":[{{"type":"text","text":{}}}],"venue":{{{venue}}}}}"#,
                    serde_json::to_string(words).expect("字")
                );
                ("message.user", MEMBERS[dice.below(3) as usize], body)
            }
            6 => (
                "venue.delivered",
                BRIDGE,
                format!(
                    r#"{{"line":"01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91","turn":3,"to":[],"msg":"{}","text":"在"}}"#,
                    id(dice)
                ),
            ),
            7 => (
                "venue.recalled",
                BRIDGE,
                format!(
                    r#"{{"msg":"{}","by":"qq:2001{}"}}"#,
                    id(dice),
                    7 + dice.below(2)
                ),
            ),
            8 => (
                "message.user",
                MEMBERS[2],
                r#"{"blocks":[{"type":"text","text":"不是场所的"}]}"#.to_string(),
            ),
            _ => (
                "turn.ended",
                r#"{"kind":"kernel"}"#,
                r#"{"reason":"completed"}"#.to_string(),
            ),
        };
        lines.push(format!(
            r#"{{"seq":{seq},"at":"2026-09-25T07:00:{:02}.000Z","kind":"{kind}","by":{by},"body":{body}}}"#,
            seq % 60
        ));
    }
    lines
        .iter()
        .map(|line| Event::from_line(line).expect("合写法"))
        .collect()
}

/// 窗口各取几样：`count` 1 到 100，步长没有、1、2、40；步长 0（照没有）、80 各一样。
fn windows() -> Vec<Window> {
    let mut all = vec![
        Window {
            count: 20,
            grow: Some(0),
        },
        Window {
            count: 20,
            grow: Some(80),
        },
    ];
    for count in [1, 3, 20, 100] {
        for grow in [None, Some(1), Some(2), Some(40)] {
            all.push(Window { count, grow });
        }
    }
    all
}

/// 端点那样只交最后 [`Window::most`] 条：撤回、引用照整份挑。
fn trimmed(
    history: &History,
    msg: Seq,
    window: Window,
    texts: &crate::texts::GroupChat,
) -> Option<crate::group::Records> {
    let events = history.events();
    let at = events.binary_search_by_key(&msg, |event| event.seq).ok()?;
    let before = &events[..at];
    let lines: Vec<&Event> = before.iter().filter(|event| on_record(event)).collect();
    let last = &lines[lines.len().saturating_sub(window.most())..];
    let recalled: BTreeMap<&str, &ExternalId> = before
        .iter()
        .filter_map(|event| match &event.body {
            miyu_kernel::event::Body::VenueRecalled(recalled) => {
                Some((recalled.msg.as_str(), &recalled.by))
            }
            _ => None,
        })
        .collect();
    let reply_to = match &events[at].body {
        miyu_kernel::event::Body::MessageUser(said) => {
            said.venue.as_ref().and_then(|venue| venue.reply_to.clone())
        }
        _ => None,
    };
    let quote = reply_to.and_then(|reply_to| {
        before
            .iter()
            .rev()
            .find(|event| quoted_as(event) == Some(reply_to.as_str()))
    });
    let picked = Picked {
        judged: &events[at],
        before: lines.len(),
        last,
        recalled: &recalled,
        quote,
        unseen: &std::collections::BTreeSet::new(),
    };
    records_from(&picked, window, texts, None)
}

/// 平时每份比前几段。
const QUICK: u64 = 50;

/// 一份的前 [`QUICK`] 段。
fn quick(seeds: std::ops::RangeInclusive<u64>) -> std::ops::RangeInclusive<u64> {
    *seeds.start()..=(*seeds.start() + QUICK - 1).min(*seeds.end())
}

/// 第 `seeds` 段的每一条、每一样窗口，新旧比。
fn compare(seeds: std::ops::RangeInclusive<u64>) {
    let texts = group_texts(480).group.expect("群会话");
    let forward = forward_texts();
    let windows = windows();
    // 全量每份五百段、一万条以上；平时照段数折。
    let least = 20 * seeds.clone().count();
    let mut judged = 0;
    for seed in seeds {
        let mut dice = Dice(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut history = History::whole();
        for event in chat(&mut dice) {
            history.append(event);
        }
        let last = history.events().last().map_or(0, |event| event.seq.get());
        for msg in (1..=last + 1).filter_map(Seq::new) {
            for &window in &windows {
                let forward = (msg.get() % 2 == 0).then_some(&forward);
                let new = records_in(&history, msg, window, &texts, forward);
                let old = old::records_in(&history, msg, window, &texts, forward);
                assert_eq!(new, old, "种子 {seed} 第 {msg} 条 {window:?}");
                if forward.is_none() {
                    assert_eq!(
                        trimmed(&history, msg, window, &texts),
                        old,
                        "只交最后几条：种子 {seed} 第 {msg} 条 {window:?}"
                    );
                }
                judged += usize::from(old.is_some());
            }
        }
    }
    assert!(judged > least, "判过的太少：{judged}");
}

// 三千段分六份，并着跑：平时每份只比前几段，全量是 `_for_longer` 那六个（`#[ignore]`，CI 的 randomized 那一项用
// `--ignored for_longer` 跑，施工 CI-2；这一层是纯逻辑，不读环境变量）。
#[test]
fn chats_render_the_same_as_before_1() {
    compare(quick(1..=500));
}

#[test]
fn chats_render_the_same_as_before_2() {
    compare(quick(501..=1000));
}

#[test]
fn chats_render_the_same_as_before_3() {
    compare(quick(1001..=1500));
}

#[test]
fn chats_render_the_same_as_before_4() {
    compare(quick(1501..=2000));
}

#[test]
fn chats_render_the_same_as_before_5() {
    compare(quick(2001..=2500));
}

#[test]
fn chats_render_the_same_as_before_6() {
    compare(quick(2501..=3000));
}

#[test]
#[ignore = "全量，CI 的 randomized 那一项用 --ignored 跑（施工 CI-2）"]
fn chats_render_the_same_as_before_1_for_longer() {
    compare(1..=500);
}

#[test]
#[ignore = "全量，CI 的 randomized 那一项用 --ignored 跑（施工 CI-2）"]
fn chats_render_the_same_as_before_2_for_longer() {
    compare(501..=1000);
}

#[test]
#[ignore = "全量，CI 的 randomized 那一项用 --ignored 跑（施工 CI-2）"]
fn chats_render_the_same_as_before_3_for_longer() {
    compare(1001..=1500);
}

#[test]
#[ignore = "全量，CI 的 randomized 那一项用 --ignored 跑（施工 CI-2）"]
fn chats_render_the_same_as_before_4_for_longer() {
    compare(1501..=2000);
}

#[test]
#[ignore = "全量，CI 的 randomized 那一项用 --ignored 跑（施工 CI-2）"]
fn chats_render_the_same_as_before_5_for_longer() {
    compare(2001..=2500);
}

#[test]
#[ignore = "全量，CI 的 randomized 那一项用 --ignored 跑（施工 CI-2）"]
fn chats_render_the_same_as_before_6_for_longer() {
    compare(2501..=3000);
}

#[test]
fn the_most_a_window_shows() {
    let most = |count, grow| Window { count, grow }.most();
    assert_eq!(most(20, None), 20);
    assert_eq!(most(20, Some(40)), 59);
    assert_eq!(most(1, Some(1)), 1);
    assert_eq!(most(100, Some(0)), 100, "步长 0 照没有");
    for window in windows() {
        for n in 0..300 {
            assert!(n - window.start(n) <= window.most(), "{window:?} {n}");
        }
    }
}
