//! 工具范围只能收窄的复核（施工 XG-2，`docs/designs/64-工具职能.md` 第五节、第十节第 2 条；测试会话 e0 的对抗式复核）：
//! 五种范围各一件，加一件声明 `any` 但只给本机的，挨个放进五种会话（本机有人看着、本机没人看着、第一层子会话、到了层数上限的
//! 子会话、群），工具面照会话的种类收窄：声明的范围放不进会话本来不给的地方，`any` 也照旧受 `venues` 管。

use miyu_kernel::id::{SessionId, VenueId};
use miyu_kernel::tool::Access;
use miyu_policy::{JOB_DEPTH, Snapshot};
use miyu_session::testkit::Script;
use miyu_session::{Handle, Lineage};
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

/// 五种范围各一件、一件声明 `any` 但只给本机的。
fn tools() -> Catalog {
    let tools: Vec<std::sync::Arc<dyn miyu_tool::Tool>> = vec![
        Fake::scoped("s_any", Access::Write, Scope::Any, Act::Echo),
        Fake::scoped("s_local", Access::Write, Scope::Local, Act::Echo),
        Fake::scoped("s_main", Access::Write, Scope::LocalMain, Act::Echo),
        Fake::scoped(
            "s_attended",
            Access::Write,
            Scope::LocalMainAttended,
            Act::Echo,
        ),
        Fake::scoped("s_spawn", Access::Write, Scope::Spawn, Act::Echo),
        Fake::in_venues(
            "home_any",
            Access::Write,
            Venues {
                local: true,
                ..Venues::default()
            },
            Act::Echo,
        ),
    ];
    Catalog::new(tools).expect("都合写法")
}

/// 第 `depth` 层的子会话。
fn child(depth: u32) -> Lines {
    Lines {
        lineage: Some(Lineage {
            parent: SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d99").expect("合写法"),
            depth,
            workspace_from: None,
        }),
        ..Lines::default()
    }
}

#[tokio::test]
async fn a_declared_scope_never_opens_a_kind_of_session_that_is_closed() {
    let home = Home::new();
    let catalog = tools();
    let unattended = Opening {
        attended: false,
        ..Opening::default()
    };
    let group = Lines {
        venue: VenueId::parse("qq:group:1").expect("场所合写法"),
        group: true,
        ..Lines::default()
    };
    let all = [
        "s_any",
        "s_local",
        "s_main",
        "s_attended",
        "s_spawn",
        "home_any",
    ];
    let cases: [(&str, Opening, Lines, &[&str]); 5] = [
        (
            "本机有人看着的主会话",
            Opening::default(),
            Lines::default(),
            &all,
        ),
        (
            "本机没人看着的主会话",
            unattended,
            Lines::default(),
            &["s_any", "s_local", "s_main", "s_spawn", "home_any"],
        ),
        (
            "第一层子会话",
            Opening::default(),
            child(1),
            &["s_any", "s_local", "s_spawn", "home_any"],
        ),
        (
            "到了层数上限的子会话",
            Opening::default(),
            child(JOB_DEPTH),
            &["s_any", "s_local", "home_any"],
        ),
        ("群", Opening::default(), group, &["s_any"]),
    ];
    for (kind, opening, lines, wanted) in cases {
        let handle = home
            .create_full(&Script::new([]), &catalog, opening, lines)
            .await;
        let mut got: Vec<String> = face(&home, &handle)
            .into_iter()
            .filter(|name| all.contains(&name.as_str()))
            .collect();
        got.sort();
        let mut wanted: Vec<String> = wanted.iter().map(|name| name.to_string()).collect();
        wanted.sort();
        assert_eq!(got, wanted, "{kind}");
    }
}
