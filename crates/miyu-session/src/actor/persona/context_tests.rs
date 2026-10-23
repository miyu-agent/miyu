//! system 拆开（施工 CX-1 上）：快照里单存、拼好的 system 里找得到的几块各算一份，找不到的不认，剩下的都是其他。

use miyu_kernel::estimate;

use super::context::{parts_of, relaid, split};
use super::test_support::{setup, write};
use crate::SystemPart;
use crate::snapshot::{build, laid, pieces};

#[test]
fn the_system_splits_into_stored_pieces_and_the_rest() {
    let (_scratch, _root, mut refresh) = setup("context-split", "");
    let snapshot = &mut refresh.snapshot;
    let skills = "技能：写周报、查天气、订闹钟。\n";
    let profile = "关于你：叫我小林就好。\n";
    let appended = "扩展说的一段，放在最后面。\n";
    snapshot.system = format!("你是美羽，一个很会聊天的人。\n{skills}{profile}{appended}");
    snapshot.skills = Some(skills.to_string());
    snapshot.profile = Some(profile.to_string());
    snapshot.project = Some("项目的提示词，后来改了，拼好的里面没有。\n".to_string());
    snapshot.knowledge = Some(String::new());
    snapshot
        .appended
        .insert("weather".to_string(), appended.to_string());
    let total = estimate::text(&snapshot.system);
    let parts = split(snapshot, total);
    let known: u64 = [skills, profile, appended]
        .into_iter()
        .map(estimate::text)
        .sum();
    assert_eq!(
        parts,
        [
            (SystemPart::Skills, estimate::text(skills)),
            (SystemPart::Profile, estimate::text(profile)),
            (
                SystemPart::Appended("weather".to_string()),
                estimate::text(appended)
            ),
            (SystemPart::Other, total - known),
        ],
        "找不到的项目、空的知识库不认"
    );
    assert_eq!(parts.iter().map(|(_, tokens)| tokens).sum::<u64>(), total);
}

/// 已有的会话照快照原来的料重拼（施工 CX-1 下）：和快照逐字节一样，交一块块在哪，连起来正好是整段；对不上的没有。
#[test]
fn an_unchanged_session_is_laid_out_block_by_block() {
    let (_scratch, _root, mut refresh) = setup("context-laid", "");
    let layout = relaid(&refresh).expect("没改过：对得上");
    let system = &refresh.snapshot.system;
    let mut at = 0;
    for (_, range) in &layout {
        assert_eq!(range.start, at, "{layout:?}");
        at = range.end;
    }
    assert_eq!(at, system.len(), "连起来是整段");
    let parts: Vec<&SystemPart> = layout.iter().map(|(part, _)| part).collect();
    assert_eq!(parts[0], &SystemPart::Persona, "{parts:?}");
    assert_eq!(&system[layout[0].1.clone()], "You are Miyu.");
    assert!(parts.contains(&&SystemPart::Core), "{parts:?}");
    refresh
        .snapshot
        .system
        .push_str("\n\n以前的核心多写的一句。");
    assert!(relaid(&refresh).is_none(), "对不上的不交");
}

/// 拼的每一步各成一块（施工 CX-1 下）：子会话的场所说明、技能、扩展追加的几段照包的先后各一块（空的那段不算），每块就是它那一截；
/// 照它拆的加起来是整段估的。
#[test]
fn each_step_of_composing_is_its_own_block() {
    let (_scratch, _root, mut refresh) = setup("context-steps", "");
    refresh.child = true;
    refresh.snapshot.skills = Some("技能：写周报。\n".to_string());
    for (id, text) in [
        ("a-pack", "甲说的一段。\n"),
        ("b-pack", "  \n"),
        ("c-pack", "丙说的。"),
    ] {
        refresh
            .snapshot
            .appended
            .insert(id.to_string(), text.to_string());
    }
    let parts = parts_of(&refresh).expect("有料");
    let (snapshot, layout) = laid(&refresh.resources, parts).expect("拼得成");
    let layout = layout.expect("都是往后接的");
    let block = |part: &SystemPart| {
        layout
            .iter()
            .find(|(found, _)| found == part)
            .map(|(_, range)| snapshot.system[range.clone()].trim_start().to_string())
    };
    assert_eq!(
        block(&SystemPart::Skills).as_deref(),
        Some("技能：写周报。")
    );
    assert!(
        block(&SystemPart::Venue).is_some_and(|venue| !venue.is_empty()),
        "{layout:?}"
    );
    assert_eq!(
        block(&SystemPart::Appended("a-pack".into())).as_deref(),
        Some("甲说的一段。")
    );
    assert_eq!(
        block(&SystemPart::Appended("b-pack".into())),
        None,
        "空的不算"
    );
    assert_eq!(
        block(&SystemPart::Appended("c-pack".into())).as_deref(),
        Some("丙说的。")
    );
    assert_eq!(
        layout.last().map(|(_, range)| range.end),
        Some(snapshot.system.len())
    );
    let total = estimate::text(&snapshot.system);
    let split = pieces(&layout, total);
    assert_eq!(split.iter().map(|(_, tokens)| tokens).sum::<u64>(), total);
    assert_eq!(split[0].0, SystemPart::Persona, "{split:?}");
}

/// 带角色扮演提示的人格有风格锁（施工 CX-1 下）：重拼照快照里有没有提示定，对得上，风格锁单成一块。
#[test]
fn a_roleplaying_persona_s_style_lock_is_its_own_block() {
    let (_scratch, root, mut refresh) = setup("context-roleplay", "");
    write(&root, "reminders.md", "说话要像美羽。\n");
    let mut parts = parts_of(&refresh).expect("有料");
    parts.texts = refresh.personas.find("miyu").expect("找得到 Miyu").texts;
    parts.roleplay = true;
    refresh.snapshot = build(&refresh.resources, parts).expect("拼得成");
    assert!(refresh.snapshot.reminder.is_some(), "有角色扮演提示");
    let layout = relaid(&refresh).expect("对得上");
    assert_eq!(
        layout.last().map(|(part, _)| part),
        Some(&SystemPart::StyleLock),
        "{layout:?}"
    );
}
