//! 工具照声明的范围给哪种会话（施工 XG-2 上，`docs/designs/64-工具职能.md` 第五节）：不照名字，第三方的同样认；范围只能收窄，
//! 声明了 `any` 的照旧受场所的 `venues` 管。自带的那几件的范围写在它们的说明文件里。

use std::path::Path;

use miyu_kernel::id::VenueId;
use miyu_kernel::tool::{Access, Role};
use miyu_policy::Snapshot;
use miyu_session::Handle;
use miyu_session::testkit::Script;
use miyu_store::blob::Blobs;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Scope, Venues};

use crate::support::*;

/// 造会话时定的工具面里的名字。
fn face(home: &Home, handle: &Handle) -> Vec<String> {
    let log = home.log(handle.id());
    let miyu_kernel::event::Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话：{:?}", log[0]);
    };
    let bytes = Blobs::new(home.root.blobs())
        .get(&created.policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes)
        .expect("读得懂")
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect()
}

async fn session(home: &Home, tools: &Catalog, venue: &str) -> Handle {
    home.create_full(
        &Script::new([]),
        tools,
        Opening::default(),
        Lines {
            venue: VenueId::parse(venue).expect("场所合写法"),
            group: venue != "local",
            ..Lines::default()
        },
    )
    .await
}

#[tokio::test]
async fn a_declared_scope_narrows_by_kind_of_session_not_by_name() {
    let tools: Vec<std::sync::Arc<dyn miyu_tool::Tool>> = vec![
        Fake::scoped("note_here", Access::Write, Scope::Local, Act::Echo),
        Fake::scoped("plan_here", Access::Write, Scope::LocalMain, Act::Echo),
        Fake::new("anywhere", Access::Read, Act::Echo),
        // 第三方声明 any 的写工具，只给本机：场所会话里照旧没有（范围不放宽，core1 2026-11-05）。
        Fake::in_venues(
            "home_only",
            Access::Write,
            Venues {
                local: true,
                ..Venues::default()
            },
            Act::Echo,
        ),
    ];
    let tools = Catalog::new(tools).expect("都合写法");
    let home = Home::new();
    let local = session(&home, &tools, "local").await;
    let here = face(&home, &local);
    for name in ["note_here", "plan_here", "anywhere", "home_only"] {
        assert!(
            here.iter().any(|tool| tool == name),
            "本机主会话有 {name}：{here:?}"
        );
    }
    let group = session(&home, &tools, "qq:group:1").await;
    let there = face(&home, &group);
    assert!(there.iter().any(|tool| tool == "anywhere"), "{there:?}");
    for name in ["note_here", "plan_here", "home_only"] {
        assert!(
            !there.iter().any(|tool| tool == name),
            "场所会话没有 {name}：{there:?}"
        );
    }
}

/// 自带的那几件照它们说明文件里写的范围、职能（原来照名字特判的那七件，和读文件的那件）。
#[test]
fn the_built_in_tools_declare_what_used_to_be_special_cased_by_name() {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let spec = |package: &str, name: &str| {
        miyu_tool::load::spec(&resources, package, name, Access::Read).expect("读得进")
    };
    for (package, name, scope) in [
        ("basesystem", "send_message", Scope::Local),
        ("basesystem", "session_usage", Scope::Local),
        ("basesystem", "todowrite", Scope::Local),
        ("basesystem", "sessions", Scope::LocalMain),
        ("goal", "goal", Scope::LocalMain),
        ("basesystem", "ask_user", Scope::LocalMainAttended),
        ("basesystem", "subagent", Scope::Spawn),
        ("basesystem", "shell", Scope::Any),
    ] {
        assert_eq!(spec(package, name).scope, scope, "{name}");
    }
    assert_eq!(spec("basesystem", "read").role, Some(Role::ReadFile));
    assert_eq!(spec("basesystem", "glob").role, None);
}
