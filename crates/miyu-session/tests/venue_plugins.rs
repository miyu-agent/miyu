//! 场所插件（施工 O-60，`docs/blueprint/venues.md`「场所插件」）：工具规格带 `plugin` 的，场所会话里只有那个场所开着它才给，
//! 没说过的一件都不给；开了下一轮有、关了下一轮没有；这一轮里刚关掉的再调回「暂时不可用」；同一个包里不带标记的照旧；本机
//! 会话不看这个标记。

use std::collections::BTreeSet;
use std::sync::Arc;

use miyu_kernel::block::Block;
use miyu_kernel::event::Body;
use miyu_kernel::id::VenueId;
use miyu_kernel::request::Request;
use miyu_kernel::tool::Access;
use miyu_policy::Snapshot;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Handle, VenuePlugins};
use miyu_store::blob::Blobs;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Call, Catalog, Done, Progress, Running, Spec, Tool};

use crate::support::*;

const GROUP: &str = "qq:group:1";

/// 带场所插件 `qq-admin` 的 `qq_mute`，同一份目录里不带标记的 `qq_send`、`read`，和 `extra` 里的。
fn catalog(extra: Vec<Arc<dyn Tool>>) -> Catalog {
    let mute = Fake::from_spec(
        Spec {
            plugin: Some("qq-admin".to_string()),
            ..Fake::new("qq_mute", Access::Read, Act::Echo).spec().clone()
        },
        Act::Echo,
    );
    let mut tools: Vec<Arc<dyn Tool>> = vec![
        mute,
        Fake::new("qq_send", Access::Read, Act::Echo),
        Fake::new("read", Access::Read, Act::Echo),
    ];
    tools.extend(extra);
    Catalog::new(tools).expect("都合写法")
}

fn names(request: &Request) -> Vec<&str> {
    request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect()
}

/// 造会话时记下的快照。
fn created(home: &Home, handle: &Handle) -> Snapshot {
    let log = home.log(handle.id());
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话：{:?}", log[0]);
    };
    let bytes = Blobs::new(home.root.blobs(&alice_account()))
        .get(&created.policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes).expect("读得懂")
}

/// 这一轮的工具结果里的字，照先后。
fn results(home: &Home, handle: &Handle) -> Vec<String> {
    home.log(handle.id())
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .map(|result| {
            result
                .blocks
                .iter()
                .map(|block| match block {
                    Block::Text(text) => text.text.as_str(),
                    _ => "",
                })
                .collect()
        })
        .collect()
}

fn on(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

/// 场所 `venue` 的会话。
async fn session(home: &Home, script: &Script, tools: &Catalog, venue: &str) -> Handle {
    home.create_full(
        script,
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

/// 说一句，等这一轮说完。
async fn turn(handle: &Handle, command: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, command, say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

#[tokio::test]
async fn plugin_tools_follow_what_the_venue_turns_on() {
    let home = Home::new();
    let script = Script::new([Play::Says("一。"), Play::Says("二。"), Play::Says("三。")]);
    let handle = session(&home, &script, &catalog(Vec::new()), GROUP).await;
    let venue = VenueId::parse(GROUP).expect("合写法");
    let made = created(&home, &handle);
    let face: Vec<&str> = made.tools.iter().map(|tool| tool.name.as_str()).collect();
    assert_eq!(
        face,
        ["qq_send", "read"],
        "造会话时就筛过，第一轮不用换快照"
    );
    assert!(made.plugin_tools.is_empty());
    turn(&handle, "cmd-1").await;
    home.plugins.set(venue.clone(), on(&["qq-admin"]));
    turn(&handle, "cmd-2").await;
    home.plugins.set(venue, on(&[]));
    turn(&handle, "cmd-3").await;
    let requests = script.requests();
    assert_eq!(
        names(&requests[0].1),
        ["qq_send", "read"],
        "没说过的不给，不带标记的照旧"
    );
    assert_eq!(
        names(&requests[1].1),
        ["qq_mute", "qq_send", "read"],
        "开了下一轮有"
    );
    assert_eq!(names(&requests[2].1), ["qq_send", "read"], "关了下一轮没有");
}

/// 一跑就把场所的插件全关掉的工具：模拟桥在这一轮当中说关。
struct TurnOff(VenuePlugins);

impl Tool for TurnOff {
    fn spec(&self) -> &Spec {
        static SPEC: std::sync::OnceLock<Spec> = std::sync::OnceLock::new();
        SPEC.get_or_init(|| {
            Fake::new("turn_off", Access::Read, Act::Echo)
                .spec()
                .clone()
        })
    }

    fn run(&self, _call: Call, _progress: Progress) -> Running<'_> {
        self.0
            .set(VenueId::parse(GROUP).expect("合写法"), BTreeSet::new());
        Box::pin(async { Done::ok("off") })
    }
}

#[tokio::test]
async fn a_call_after_turning_off_is_unavailable_within_the_turn() {
    let home = Home::new();
    let venue = VenueId::parse(GROUP).expect("合写法");
    home.plugins.set(venue, on(&["qq-admin"]));
    let script = Script::new([
        Play::calls(&[("turn_off", "{}")]),
        Play::calls(&[("qq_mute", "{}")]),
        Play::Says("好。"),
    ]);
    let off: Arc<dyn Tool> = Arc::new(TurnOff(home.plugins.clone()));
    let handle = session(&home, &script, &catalog(vec![off]), GROUP).await;
    turn(&handle, "cmd-1").await;
    let requests = script.requests();
    assert!(
        names(&requests[1].1).contains(&"qq_mute"),
        "这一轮的工具面不动：{:?}",
        names(&requests[1].1)
    );
    let results = results(&home, &handle);
    let text = results.last().expect("有 qq_mute 的结果");
    assert_eq!(
        text.trim_end(),
        "The tool \"qq_mute\" is not available right now.",
        "当场停，不跑"
    );
}

#[tokio::test]
async fn local_sessions_ignore_the_mark() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let handle = session(&home, &script, &catalog(Vec::new()), "local").await;
    turn(&handle, "cmd-1").await;
    assert_eq!(
        names(&script.requests()[0].1),
        ["qq_mute", "qq_send", "read"],
        "本机会话照原来的"
    );
}

/// 脚本经 `miyu call` 能调的照这一轮的工具面算：开着进了这一轮、这一轮里又关掉的，脚本再调也是暂时不可用。
#[tokio::test]
async fn a_script_cannot_reach_a_plugin_tool_turned_off_within_the_turn() {
    let home = Home::new();
    home.plugins
        .set(VenueId::parse(GROUP).expect("合写法"), on(&["qq-admin"]));
    let shell = Fake::new("shell", Access::Read, Act::Calls(vec![("qq_mute", "{}")]));
    let mute = Fake::from_spec(
        Spec {
            plugin: Some("qq-admin".to_string()),
            ..Fake::callable("qq_mute", Access::Read, Act::Echo)
                .spec()
                .clone()
        },
        Act::Echo,
    );
    let off: Arc<dyn Tool> = Arc::new(TurnOff(home.plugins.clone()));
    let tools =
        Catalog::new([shell as Arc<dyn Tool>, mute as Arc<dyn Tool>, off]).expect("都合写法");
    let script = Script::new([
        Play::calls(&[("turn_off", "{}")]),
        Play::calls(&[("shell", r#"{"command":"python3 x.py"}"#)]),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script, &tools, GROUP).await;
    turn(&handle, "cmd-1").await;
    let results = results(&home, &handle);
    assert!(
        results[1]
            .starts_with("qq_mute error text=The tool \"qq_mute\" is not available right now."),
        "脚本经 miyu call 也调不到：{results:?}"
    );
}
