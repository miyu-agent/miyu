//! 包的工具变了，下一个回合换上（施工 O-2 中）：目录换了代，提供者的工具照现在的登记加上、改掉、去掉；自带的照旧快照里的
//! 原样、不新加；只给群的不进本机的会话，拼出来一样的不换；载入的不知道是哪一代，第一个回合对
//! 一次；以前造的预设没有指纹的不跟。

use std::sync::Arc;

use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Spec, Tool, Venues};

use super::test_support::{look_now, names, setup};
use super::*;
use miyu_tool::Catalog;

const LOCAL: Venues = Venues {
    local: true,
    private: false,
    group: false,
};

const GROUP: Venues = Venues {
    local: false,
    private: false,
    group: true,
};

fn provided(name: &str, venues: Venues) -> Arc<dyn Tool> {
    Fake::in_venues(name, Access::Read, venues, Act::Echo)
}

/// 桥照 `tools` 登记一次。
fn provide(refresh: &Refresh, tools: Vec<Arc<dyn Tool>>) {
    refresh
        .tools
        .replace(|catalog| catalog.replacing("onebot", tools))
        .expect("登记得上");
}

/// 看一遍，该换：照新的快照，记下现在这一代。
fn swap(refresh: &mut Refresh) -> Snapshot {
    let Seen::Swapped(snapshot, ..) = look_now(refresh) else {
        panic!("该换");
    };
    refresh.snapshot = (*snapshot).clone();
    refresh.seen = Some(refresh.tools.edition());
    *snapshot
}

#[test]
fn provided_tools_join_change_and_leave_the_face() {
    let (_scratch, _root, mut refresh) = setup("shelf-join", "");
    provide(&refresh, vec![provided("send", LOCAL)]);
    let snapshot = swap(&mut refresh);
    assert_eq!(names(&snapshot), ["read", "remember", "send", "shell"]);
    assert!(matches!(look_now(&refresh), Seen::Same), "换过以后不再换");
    let changed = Spec {
        name: "send".to_string(),
        description: "Send, the new way.".to_string(),
        parameters: serde_json::from_str::<RawJson>(r#"{"type":"object"}"#).unwrap(),
        access: Access::Read,
        clicall: false,
        result: None,
        plugin: None,
        role: None,
        scope: miyu_tool::Scope::Any,
    };
    provide(&refresh, vec![Fake::from_spec(changed, Act::Echo)]);
    let snapshot = swap(&mut refresh);
    let send = snapshot.tools.iter().find(|tool| tool.name == "send");
    assert_eq!(
        send.map(|tool| tool.description.as_str()),
        Some("Send, the new way."),
        "改了说明的照现在的"
    );
    provide(&refresh, Vec::new());
    let snapshot = swap(&mut refresh);
    assert_eq!(
        names(&snapshot),
        ["read", "remember", "shell"],
        "去掉了的拿掉"
    );
}

#[test]
fn built_in_tools_stay_as_they_were() {
    let (_scratch, _root, mut refresh) = setup("shelf-kept", "");
    // 快照里那一件是以前的样子（程序升级过、描述改了）：目录换代时照旧留着它。
    refresh.snapshot.tools[0].description = "Read, the old way.".to_string();
    let read = refresh.snapshot.tools[0].clone();
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    refresh
        .tools
        .replace(|_| {
            Catalog::in_packages([
                (
                    "basesystem",
                    vec![fake("glob"), fake("read"), fake("shell")],
                ),
                ("memory", vec![fake("remember")]),
            ])?
            .replacing("onebot", vec![provided("send", LOCAL)])
        })
        .expect("登记得上");
    let snapshot = swap(&mut refresh);
    assert_eq!(
        names(&snapshot),
        ["read", "remember", "send", "shell"],
        "新的自带工具等预设改了才进来"
    );
    assert_eq!(snapshot.tools[0], read, "留着的那一件原样");
}

#[test]
fn a_tool_for_groups_only_leaves_a_local_session_alone() {
    let (_scratch, _root, refresh) = setup("shelf-group", "");
    provide(&refresh, vec![provided("mute", GROUP)]);
    assert!(matches!(look_now(&refresh), Seen::Same), "拼出来一样的不换");
}

#[test]
fn a_loaded_session_checks_the_catalog_once() {
    let (_scratch, _root, mut refresh) = setup("shelf-loaded", "");
    refresh.seen = None;
    // 施工 WK-2 以前造的快照：没记包（记了包的见 `a_missing_tool_of_a_builtin_package_still_here_leaves`）。
    for tool in &mut refresh.snapshot.tools {
        tool.package = None;
    }
    assert!(matches!(look_now(&refresh), Seen::Same), "目录和快照对得上");
    // 程序升级拿掉了一件自带的：载入的认不出它是哪来的，照旧留着，调到时暂时不可用（施工 4-2）。
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    refresh
        .tools
        .replace(|_| {
            Catalog::in_packages([
                ("basesystem", vec![fake("read")]),
                ("memory", vec![fake("remember")]),
            ])
        })
        .expect("登记得上");
    assert!(
        matches!(look_now(&refresh), Seen::Same),
        "目录里没有了的自带工具照旧留着"
    );
    provide(&refresh, vec![provided("send", LOCAL)]);
    refresh.seen = Some(refresh.tools.edition());
    assert!(
        matches!(look_now(&refresh), Seen::Same),
        "认得是哪一代、没换代的不看"
    );
    refresh.seen = None;
    let snapshot = swap(&mut refresh);
    assert_eq!(names(&snapshot), ["read", "remember", "send", "shell"]);
}

#[test]
fn an_older_pin_does_not_follow_the_catalog() {
    let (_scratch, _root, mut refresh) = setup("shelf-older", "");
    if let Some(pin) = refresh.snapshot.preset.as_mut() {
        pin.digest = None;
    }
    provide(&refresh, vec![provided("send", LOCAL)]);
    assert!(
        matches!(look_now(&refresh), Seen::Same),
        "P-2（中）造的快照找不回预设，不跟"
    );
}

/// 没有预设的会话（P-2 以前造的）照样跟着目录。
#[test]
fn a_session_without_a_preset_follows_the_catalog_too() {
    let (_scratch, _root, mut refresh) = setup("shelf-nopreset", "");
    let parts = Parts {
        name: refresh.snapshot.persona.clone(),
        texts: refresh.personas.find("miyu").expect("找得到 Miyu").texts,
        attended: true,
        face: refresh.snapshot.tools.clone(),
        memory: refresh.snapshot.memory.clone(),
        child: false,
        preset: None,
        roleplay: true,
        group: None,
        foreground: false,
        show_hint: true,

        knowledge: None,
        skills: None,
        profile: None,
        project: None,
        appended: Default::default(),
        bare: false,
        plugin_tools: Vec::new(),
        local: true,
        steps: miyu_policy::steps::StepLimits::of(miyu_policy::steps::Watched::Attended, None),
        search: None,
    };
    refresh.snapshot = build(&refresh.resources, parts).expect("拼得成");
    assert!(matches!(look_now(&refresh), Seen::Same), "没换代的不换");
    provide(&refresh, vec![provided("send", LOCAL)]);
    let snapshot = swap(&mut refresh);
    assert_eq!(names(&snapshot), ["read", "remember", "send", "shell"]);
}

/// 关着的包登记了工具：工具面里没有，装了没开的那一行带上它。找不回预设的快照这一行也不跟。
#[test]
fn a_provided_package_that_is_off_changes_nothing() {
    // 关着的包登记了工具：工具面不变，system 也不再提它（施工 P-OFF），不换。
    let act = || provided("act", LOCAL);
    let (_scratch, _root, refresh) = setup("shelf-off-line", "[software]\nroleplay = false\n");
    refresh
        .tools
        .replace(|catalog| catalog.replacing("roleplay", vec![act()]))
        .expect("登记得上");
    assert!(matches!(look_now(&refresh), Seen::Same));
    let (_scratch, _root, mut older) = setup("shelf-older-line", "[software]\nroleplay = false\n");
    if let Some(pin) = older.snapshot.preset.as_mut() {
        pin.digest = None;
    }
    older
        .tools
        .replace(|catalog| catalog.replacing("roleplay", vec![act()]))
        .expect("登记得上");
    assert!(matches!(look_now(&older), Seen::Same), "找不回预设的不跟");
}

/// 预设改了、同时提供者改了说明：自带的照旧快照里的，提供者的照现在的登记。
#[test]
fn a_changed_preset_takes_provided_tools_as_registered_now() {
    let (_scratch, root, mut refresh) = setup("shelf-preset", "");
    provide(&refresh, vec![provided("send", LOCAL)]);
    swap(&mut refresh);
    let changed = Spec {
        name: "send".to_string(),
        description: "Send, the new way.".to_string(),
        parameters: serde_json::from_str::<RawJson>(r#"{"type":"object"}"#).unwrap(),
        access: Access::Read,
        clicall: false,
        result: None,
        plugin: None,
        role: None,
        scope: miyu_tool::Scope::Any,
    };
    provide(&refresh, vec![Fake::from_spec(changed, Act::Echo)]);
    super::test_support::write_preset(&root, "[tools]\nshell = false\n");
    let snapshot = swap(&mut refresh);
    assert_eq!(names(&snapshot), ["read", "remember", "send"]);
    let send = snapshot.tools.iter().find(|tool| tool.name == "send");
    assert_eq!(
        send.map(|tool| tool.description.as_str()),
        Some("Send, the new way.")
    );
}

/// 升级以前造的会话、只是目录换了代（施工 P-1 三补）：工具面真的变了的换，换出来是新的核心的字；没变的（只给群的工具、
/// 关着的包进了装了没开的那一行）不换，不为升级断一次缓存。
#[test]
fn after_an_upgrade_only_a_changed_face_swaps() {
    let older = |name: &str, text: &str| {
        let (scratch, root, mut refresh) = setup(name, text);
        refresh.snapshot.core.facts.env = "<env/>\n".to_string();
        (scratch, root, refresh)
    };
    let (_scratch, _root, mut refresh) = older("upgrade-face", "");
    provide(&refresh, vec![provided("send", LOCAL)]);
    let snapshot = swap(&mut refresh);
    assert_eq!(names(&snapshot), ["read", "remember", "send", "shell"]);
    assert_ne!(snapshot.core.facts.env, "<env/>\n", "新的核心的字一起换上");
    let (_scratch, _root, refresh) = older("upgrade-group", "");
    provide(&refresh, vec![provided("mute", GROUP)]);
    assert!(matches!(look_now(&refresh), Seen::Same), "工具面没变的不换");
    let (_scratch, _root, refresh) = older("upgrade-line", "[software]\nroleplay = false\n");
    refresh
        .tools
        .replace(|catalog| catalog.replacing("roleplay", vec![provided("act", LOCAL)]))
        .expect("登记得上");
    assert!(
        matches!(look_now(&refresh), Seen::Same),
        "只有装了没开的那一行变了的不换"
    );
    let (_scratch, _root, mut refresh) = older("upgrade-loaded", "");
    refresh.seen = None;
    assert!(
        matches!(look_now(&refresh), Seen::Same),
        "载入时对一次：光是升级不换"
    );
}

/// 装上、卸掉内置包（施工 F-5 中，设计 30 第九节）：卸掉的照旧留在工具面上（调到时报已卸载），装回来的包下一个回合进来。
#[test]
fn a_builtin_package_installed_or_removed_follows_the_rules() {
    let (_scratch, _root, mut refresh) = setup("shelf-builtin", "");
    refresh
        .tools
        .replace(|catalog| Ok::<_, miyu_tool::CatalogError>(catalog.removing("memory")))
        .expect("拿得掉");
    let before = names(&refresh.snapshot)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    match look_now(&refresh) {
        Seen::Same => {}
        Seen::Swapped(snapshot, ..) => {
            assert_eq!(names(&snapshot), before, "卸掉的照旧留着");
        }
        _ => panic!("该是照旧或者换上一样的"),
    }
    assert!(refresh.tools.gone("remember"), "记下随包卸掉了");
    refresh.seen = Some(refresh.tools.edition());
    refresh
        .snapshot
        .tools
        .retain(|tool| tool.name != "remember");
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    refresh
        .tools
        .replace(|catalog| catalog.placing("memory", vec![fake("remember")]))
        .expect("装得上");
    let snapshot = swap(&mut refresh);
    assert_eq!(
        names(&snapshot),
        ["read", "remember", "shell"],
        "新装上的包下一个回合进来"
    );
}

/// 快照里冻着的一件，施工 WK-2 以前造的那样：没记包。
fn frozen(name: &str) -> miyu_policy::ToolEntry {
    miyu_policy::ToolEntry {
        name: name.to_string(),
        description: format!("{name}, the old way."),
        parameters: serde_json::from_str::<RawJson>(r#"{"type":"object"}"#).unwrap(),
        access: Access::Read,
        clicall: false,
        runs_commands: false,
        command_only: false,
        role: None,
        package: None,
    }
}

/// 载入的会话（不知道快照照的是哪一代），快照里多冻着 `extra`，目录换成 `catalog`。
fn loaded_with(refresh: &mut Refresh, extra: Vec<miyu_policy::ToolEntry>, catalog: Catalog) {
    let mut tools = refresh.snapshot.tools.clone();
    tools.extend(extra);
    refresh.snapshot = refresh.snapshot.clone().with_tools(tools);
    refresh.seen = None;
    refresh
        .tools
        .replace(|_| Ok::<_, miyu_tool::CatalogError>(catalog))
        .expect("登记得上");
}

/// 基础系统两件、记忆一件，循环这个包是改名以后的 `schedule_next`。
fn renamed_catalog() -> Catalog {
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    Catalog::in_packages([
        ("basesystem", vec![fake("read"), fake("shell")]),
        ("memory", vec![fake("remember")]),
        ("loop", vec![fake(miyu_tool::SCHEDULE_NEXT)]),
    ])
    .expect("合写法")
}

/// 改名以前造的会话（施工 WK-2）：快照里冻着 `schedule_wakeup`，载入后第一个回合换成目录里新名字的那件，照现在的目录拿；
/// 旧名字不留。
#[test]
fn a_renamed_tool_moves_to_its_new_name() {
    let (_scratch, _root, mut refresh) = setup("rename-moves", "");
    loaded_with(
        &mut refresh,
        vec![frozen("schedule_wakeup")],
        renamed_catalog(),
    );
    let snapshot = swap(&mut refresh);
    assert_eq!(
        names(&snapshot),
        ["read", "remember", miyu_tool::SCHEDULE_NEXT, "shell"]
    );
    let next = snapshot
        .tools
        .iter()
        .find(|tool| tool.name == miyu_tool::SCHEDULE_NEXT)
        .expect("有新的");
    assert_eq!(next.package.as_deref(), Some("loop"), "照现在的目录拿");
    assert!(matches!(look_now(&refresh), Seen::Same), "换过以后不再换");
}

/// 改了名、预设现在不给新的那件（施工 WK-2）：旧的新的都没有。
#[test]
fn a_renamed_tool_the_preset_no_longer_offers_leaves() {
    let (_scratch, _root, mut refresh) = setup("rename-off", "[software]\nloop = false\n");
    loaded_with(
        &mut refresh,
        vec![frozen("schedule_wakeup")],
        renamed_catalog(),
    );
    let snapshot = swap(&mut refresh);
    assert_eq!(names(&snapshot), ["read", "remember", "shell"]);
}

/// 快照里新旧两个名字都有的（施工 WK-2）：换完只有一件新的，不重。
#[test]
fn a_renamed_tool_already_on_the_face_is_not_doubled() {
    let (_scratch, _root, mut refresh) = setup("rename-twice", "");
    let both = vec![frozen("schedule_wakeup"), frozen(miyu_tool::SCHEDULE_NEXT)];
    loaded_with(&mut refresh, both, renamed_catalog());
    let snapshot = swap(&mut refresh);
    assert_eq!(
        names(&snapshot),
        ["read", "remember", miyu_tool::SCHEDULE_NEXT, "shell"]
    );
}

/// 目录里没了、没改过名、快照没记包的（施工 4-2，WK-2 以前造的快照）：照旧留着。新名字不在目录里的改名也不换，同样留着。
#[test]
fn a_missing_tool_without_a_package_stays() {
    let (_scratch, _root, mut refresh) = setup("rename-unknown", "");
    let extra = vec![frozen("gone_tool"), frozen("schedule_wakeup")];
    loaded_with(&mut refresh, extra, super::test_support::catalog());
    match look_now(&refresh) {
        Seen::Same => {}
        Seen::Swapped(snapshot, ..) => assert_eq!(
            names(&snapshot),
            ["gone_tool", "read", "remember", "schedule_wakeup", "shell"]
        ),
        _ => panic!("该是照旧或者换上一样的"),
    }
}

/// 快照记着包、那个内置包还在目录里、这件没了的（施工 WK-2）：程序升级拿掉的，拿掉。
#[test]
fn a_missing_tool_of_a_builtin_package_still_here_leaves() {
    let (_scratch, _root, mut refresh) = setup("rename-builtin", "");
    let shell = refresh
        .snapshot
        .tools
        .iter()
        .find(|tool| tool.name == "shell");
    assert_eq!(
        shell.and_then(|tool| tool.package.as_deref()),
        Some("basesystem"),
        "组工具面时记下包"
    );
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    let upgraded = Catalog::in_packages([
        ("basesystem", vec![fake("read")]),
        ("memory", vec![fake("remember")]),
    ])
    .expect("合写法");
    loaded_with(&mut refresh, Vec::new(), upgraded);
    let snapshot = swap(&mut refresh);
    assert_eq!(names(&snapshot), ["read", "remember"]);
}

/// 快照记着包、可那个包是提供者登记的，或者这件随包卸掉了（施工 WK-2）：照以前的规矩留着（扩展还没登记上来、调到时报已卸载）。
#[test]
fn a_missing_tool_of_a_provider_or_a_removed_package_stays() {
    let (_scratch, _root, mut refresh) = setup("rename-provider", "");
    provide(
        &refresh,
        vec![provided("send", LOCAL), provided("poke", LOCAL)],
    );
    let snapshot = swap(&mut refresh);
    assert_eq!(
        names(&snapshot),
        ["poke", "read", "remember", "send", "shell"]
    );
    let catalog = refresh
        .tools
        .current()
        .replacing("onebot", vec![provided("send", LOCAL)])
        .expect("登记得上")
        .removing("memory");
    loaded_with(&mut refresh, Vec::new(), catalog);
    match look_now(&refresh) {
        Seen::Same => {}
        Seen::Swapped(snapshot, ..) => assert_eq!(
            names(&snapshot),
            ["poke", "read", "remember", "send", "shell"]
        ),
        _ => panic!("该是照旧或者换上一样的"),
    }
}
