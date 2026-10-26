//! 删哪几趟：同一间的留最新的几个（它自己也算），别间的、在忙的不删；它不在列表里的一个都不删。

use miyu_kernel::time::Timestamp;

use super::*;

/// 编号照先后（`n` 越大越新）、那一间、忙不忙。
fn listed(n: u8, persona: &str, busy: bool) -> Listed {
    Listed {
        id: id(n),
        oneshot: false,
        parent: None,
        title: String::new(),
        named: false,
        pinned: false,
        cwd: "~".to_string(),
        busy,
        last_active: Timestamp::from_unix_millis(0).expect("是时刻"),
        preview: String::new(),
        venue: "local".to_string(),
        persona: None,
        preset: None,
        project: None,
        tokens: 0,
        context: None,
        context_model: None,
        window: None,
        default_workspace: None,
        dream: Some(Dream {
            persona: Some(persona.to_string()),
            session: None,
            manual: false,
            from: None,
        }),
        origin: None,
        forked: false,
    }
}

fn id(n: u8) -> SessionId {
    SessionId::parse(&format!("01a15340-b949-7220-aeb1-30f2899eb5{n:02}")).expect("合写法")
}

#[test]
fn the_newest_of_the_same_room_are_kept() {
    // 从新到旧：5、4（别间）、3（在忙）、2、1。
    let all = [
        listed(5, "a", false),
        listed(4, "b", false),
        listed(3, "a", true),
        listed(2, "a", false),
        listed(1, "a", false),
    ];
    assert_eq!(doomed(&all, &id(5), 2), [id(2), id(1)]);
    assert_eq!(doomed(&all, &id(5), 1), [id(2), id(1)], "在忙的不删");
    assert_eq!(
        doomed(&all, &id(5), 0),
        [id(5), id(2), id(1)],
        "0 连它一起删"
    );
    assert_eq!(doomed(&all, &id(5), 20), Vec::<SessionId>::new());
    assert_eq!(
        doomed(&all, &id(9), 0),
        Vec::<SessionId>::new(),
        "它不在的不删"
    );
}
