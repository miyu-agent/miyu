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
    let said =
        |id: &str, file: Option<&PersonaFile>| persona_choice(id.to_string(), file).description;
    let both = file(
        "[persona]\nname = { zh = \"猫\", en = \"Cat\" }\nsummary = { zh = \"喵\", en = \"Purrs\\n  a lot\" }\n",
    );
    assert_eq!(
        said("cat", Some(&both)).as_deref(),
        Some("Cat — Purrs a lot")
    );
    let named = file("[persona]\nname = \"美羽\"\n");
    assert_eq!(
        said("miyu", Some(&named)).as_deref(),
        Some("美羽"),
        "只有中文的照给"
    );
    let summed = file("[persona]\nsummary = \"Quiet.\"\n");
    assert_eq!(said("quiet", Some(&summed)).as_deref(), Some("Quiet."));
    let blank = file("[persona]\nname = \"Bare\"\nsummary = \"\"\n");
    assert_eq!(
        said("bare", Some(&blank)).as_deref(),
        Some("Bare"),
        "空的简单描述当没写"
    );
    assert_eq!(said("plain", Some(&PersonaFile::default())), None);
    assert_eq!(said("broken", None), None, "叠不成的只有编号");
    assert_eq!(persona_choice("cat".to_string(), None).name, "cat");
}
