//! system 拆开（施工 CX-1 上）：快照里单存、拼好的 system 里找得到的几块各算一份，找不到的不认，剩下的都是其他。

use miyu_kernel::estimate;

use super::context::split;
use super::test_support::setup;
use crate::SystemPart;

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
