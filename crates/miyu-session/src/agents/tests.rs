//! 工具面照会话在哪挑提供者的工具（施工 O-2 上）：给本机的只进本机的会话，给私聊、群的只进那一种场所会话；核心自带的照旧。

use miyu_config::Values;
use miyu_kernel::tool::Access;
use miyu_tool::Venues;
use miyu_tool::testkit::{Act, Fake};

use super::*;

fn catalog() -> Catalog {
    let only = |local, private, group| Venues {
        local,
        private,
        group,
    };
    Catalog::in_packages([
        (
            "basesystem",
            vec![Fake::new("read", Access::Read, Act::Echo) as Arc<dyn miyu_tool::Tool>],
        ),
        (
            "onebot",
            vec![
                Fake::in_venues(
                    "send_any",
                    Access::Outbound,
                    only(true, false, false),
                    Act::Echo,
                ),
                Fake::in_venues(
                    "skip_reply",
                    Access::Read,
                    only(false, true, true),
                    Act::Echo,
                ),
                Fake::in_venues(
                    "mute",
                    Access::Outbound,
                    only(false, false, true),
                    Act::Echo,
                ),
            ],
        ),
    ])
    .expect("合写法")
}

fn names(venue: &str, group: bool) -> Vec<String> {
    let venue = VenueId::parse(venue).expect("合写法");
    Agents::face(
        &catalog(),
        Site {
            venue: &venue,
            group,
        },
        None,
        &Offers::of(&Values::default(), Vec::new()),
        false,
        crate::agents::Holds {
            memory: MemoryScope::Off,
            knowledge: crate::agents::Reach::Save,
        },
        None,
    )
    .into_iter()
    .map(|entry| entry.name)
    .collect()
}

#[test]
fn provided_tools_go_to_the_kind_of_session_they_are_for() {
    assert_eq!(names("local", false), ["read", "send_any"]);
    assert_eq!(names("qq:private:10001", false), ["read", "skip_reply"]);
    assert_eq!(names("qq:group:1", true), ["mute", "read", "skip_reply"]);
}

/// 结果没人要了时停哪几个（施工 7-5 补）：派出去的子代理，后台命令、没有子会话的不算。
#[test]
fn only_spawned_subagents_count_as_unclaimed() {
    let job = |n: &str| JobId::parse(n).expect("合写法");
    let child = SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91").expect("合写法");
    let started = |n: &str, what: JobKind, session: Option<SessionId>| {
        Effect::JobStarted(JobStarted {
            foreground: false,
            fork: false,
            job: job(n),
            what,
            title: "查".to_string(),
            session,
        })
    };
    let effects = [
        started("j1", JobKind::Command, None),
        started("j2", JobKind::Agent, Some(child.clone())),
        started("j3", JobKind::Agent, None),
    ];
    assert_eq!(spawned_in(&effects), [(job("j2"), child)]);
    assert!(spawned_in(&[]).is_empty());
}

/// 派子代理挑人格那一行（施工 P-10）：名字和简单描述都有的两样都给，缺一样给另一样，都没有的只剩编号；挑英文的，换行并成一行。
#[test]
fn a_persona_choice_carries_its_name_and_summary() {
    let file = |toml: &str| miyu_policy::persona::read_toml(toml).expect("合写法");
    let said = |id: &str, file: &PersonaFile| persona_choice(id.to_string(), file).description;
    let both = file(
        "[persona]\nname = { zh = \"猫\", en = \"Cat\" }\nsummary = { zh = \"喵\", en = \"Purrs\\n  a lot\" }\n",
    );
    assert_eq!(said("cat", &both).as_deref(), Some("Cat — Purrs a lot"));
    let named = file("[persona]\nname = \"美羽\"\n");
    assert_eq!(
        said("miyu", &named).as_deref(),
        Some("美羽"),
        "只有中文的照给"
    );
    let summed = file("[persona]\nsummary = \"Quiet.\"\n");
    assert_eq!(said("quiet", &summed).as_deref(), Some("Quiet."));
    let blank = file("[persona]\nname = \"Bare\"\nsummary = \"\"\n");
    assert_eq!(
        said("bare", &blank).as_deref(),
        Some("Bare"),
        "空的简单描述当没写"
    );
    assert_eq!(said("plain", &PersonaFile::default()), None);
    assert_eq!(persona_choice("cat".to_string(), &both).name, "cat");
}

/// 一个基础系统：`shell`，和一件脚本里能不能调照 `clicall` 的 `web_fetch`。
fn scripting(clicall: bool) -> Catalog {
    let fetch =
        miyu_tool::Tool::spec(Fake::new("web_fetch", Access::Network, Act::Echo).as_ref()).clone();
    Catalog::in_packages([(
        "basesystem",
        vec![
            Fake::new(miyu_tool::SHELL, Access::Execute, Act::Echo) as Arc<dyn miyu_tool::Tool>,
            Fake::from_spec(miyu_tool::Spec { clicall, ..fetch }, Act::Echo),
        ],
    )])
    .expect("合写法")
}

/// 照目录 `catalog`、预设 `preset`（TOML 原文，没有的是没预设）拼本机会话的工具面。
fn scripted_face(catalog: &Catalog, preset: Option<&str>) -> Vec<miyu_policy::ToolEntry> {
    let venue = VenueId::parse("local").expect("合写法");
    let file = preset.map(|toml| miyu_policy::preset::read(toml).expect("合写法"));
    Agents::face(
        catalog,
        Site {
            venue: &venue,
            group: false,
        },
        None,
        &Offers::of(&Values::default(), Vec::new()),
        false,
        crate::agents::Holds {
            memory: MemoryScope::Off,
            knowledge: crate::agents::Reach::Save,
        },
        file.as_ref().map(|file| (file, None)),
    )
}

/// 「脚本调工具」照预设开关（施工 M-6）：没有预设的、整个基础系统开着的、没写的照开的预设里能调；只开了别的功能、没写的照关的
/// 预设（极简开发那样）一件都不能调，工具照旧在。
#[test]
fn a_preset_without_tools_from_scripts_makes_nothing_scriptable() {
    let catalog = scripting(true);
    let scriptable = |preset: Option<&str>| -> Vec<String> {
        scripted_face(&catalog, preset)
            .into_iter()
            .filter(|entry| entry.clicall)
            .map(|entry| entry.name)
            .collect()
    };
    assert_eq!(scriptable(None), ["web_fetch"], "没有预设的照开着");
    let whole = "[preset]\nunlisted = \"off\"\n[software]\nbasesystem = true\n";
    assert_eq!(
        scriptable(Some(whole)),
        ["web_fetch"],
        "常规开发：整个基础系统开着"
    );
    let everything = "[preset]\nunlisted = \"on\"\n";
    assert_eq!(scriptable(Some(everything)), ["web_fetch"], "全部功能");
    let minimal = "[preset]\nunlisted = \"off\"\n[features]\ncommands = true\nweb = true\n";
    assert!(
        scriptable(Some(minimal)).is_empty(),
        "极简开发：没写「脚本调工具」，关着"
    );
    let off = "[preset]\nunlisted = \"on\"\n[features]\nclicall = false\n";
    assert!(scriptable(Some(off)).is_empty(), "明着关掉");
}

/// 关了的会话回到完全没有「脚本调工具」时的样子（施工 M-6，核心主会话要的）：工具面（发出去的工具说明、参数）和一件都不能在
/// 脚本里调的目录拼出来的一字不差，`shell` 的说明末尾不接那一段，system 那一句也不要（`tells`）。开着的照旧接上。
#[test]
fn switched_off_the_face_is_the_same_as_without_tools_from_scripts() {
    let off = "[preset]\nunlisted = \"on\"\n[features]\nclicall = false\n";
    let switched = scripted_face(&scripting(true), Some(off));
    let never = scripted_face(&scripting(false), Some(off));
    assert_eq!(switched, never);
    assert!(!crate::agents::clicall::tells(&switched));
    let on = scripted_face(&scripting(true), None);
    assert!(
        crate::agents::clicall::tells(&on),
        "开着的照旧带 system 那一句"
    );
}
