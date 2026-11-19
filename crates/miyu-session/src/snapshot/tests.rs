//! 拼快照（施工 VN-1）：本机会话带「这一轮的话从哪来」那几句，`head/` 下一种头一句；场所会话不带。

use std::path::Path;

use miyu_policy::PersonaTexts;
use miyu_policy::steps::{StepLimits, Watched};
use miyu_store::resources::ResourceRoot;

use super::*;

/// 出厂资源上拼一份最小的快照：`local` 是不是本机会话。
fn built(local: bool) -> Snapshot {
    let resources = ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let parts = Parts {
        name: None,
        texts: PersonaTexts::default(),
        attended: true,
        face: Vec::new(),
        memory: None,
        child: false,
        preset: None,
        roleplay: false,
        group: None,
        foreground: false,
        show_hint: false,
        knowledge: None,
        skills: None,
        profile: None,
        project: None,
        appended: Default::default(),
        bare: false,
        plugin_tools: Vec::new(),
        local,
        steps: StepLimits::of(Watched::Attended, None),
        search: None,
    };
    build(&resources, parts).expect("拼得成")
}

#[test]
fn a_local_session_carries_a_line_per_head_kind() {
    let channel = built(true).core.facts.channel.expect("本机会话带着");
    let kinds: Vec<&str> = channel.heads.keys().map(String::as_str).collect();
    assert_eq!(kinds, ["cli", "shellhook", "tui", "web"]);
    for text in channel
        .heads
        .values()
        .chain([&channel.other, &channel.nobody])
    {
        assert!(
            text.starts_with("<channel>") && text.ends_with("</channel>\n"),
            "{text}"
        );
    }
}

#[test]
fn a_venue_session_does_not() {
    assert_eq!(built(false).core.facts.channel, None);
}
