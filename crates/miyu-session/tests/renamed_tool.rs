//! 改名以前造的会话（施工 WK-2，`miyu_tool::RENAMED`）：快照里冻着旧名字 `schedule_wakeup`。载入时照样认 `/loop`；下一个回合
//! 开始工具面换成新名字 `schedule_next`，发给模型的 tools 里只有新名字。老快照照 WK-2 以前的样子造：新会话的快照里把那件改回
//! 旧名字、各件都不记包，再把日志里造会话那一条指过去。

use std::sync::Arc;

use miyu_kernel::event::Body;
use miyu_kernel::tool::Access;
use miyu_policy::Snapshot;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, SCHEDULE_NEXT, Tool};

use crate::support::*;

/// 基础系统一件、循环这个包一件（改名以后的名字）。
fn catalog() -> Catalog {
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    Catalog::in_packages([
        ("basesystem", vec![fake("read")]),
        ("loop", vec![fake(SCHEDULE_NEXT)]),
    ])
    .expect("合写法")
}

/// 一次请求发给模型的工具名。
fn sent(script: &Script, at: usize) -> Vec<String> {
    script.requests()[at]
        .1
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect()
}

#[tokio::test]
async fn a_session_frozen_with_the_old_name_loops_and_moves_to_the_new_one() {
    let home = Home::new();
    let tools = catalog();
    let first = Script::new([Play::Says("好。")]);
    let handle = home
        .create_full(&first, &tools, Opening::default(), Lines::default())
        .await;
    assert!(handle.loops(), "新会话工具面上有 schedule_next");
    let session = handle.id().clone();
    stop(&handle).await;

    // 改回旧名字、不记包，就是 WK-2 以前造的那份。
    let blobs = Blobs::new(home.root.blobs());
    let log = home.log(&session);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话：{:?}", log[0]);
    };
    let bytes = blobs.get(&created.policy).expect("快照在 blob 里");
    let mut snapshot = Snapshot::from_bytes(&bytes).expect("读得懂");
    for tool in &mut snapshot.tools {
        if tool.name == SCHEDULE_NEXT {
            tool.name = "schedule_wakeup".to_string();
        }
        tool.package = None;
    }
    let tools_now = std::mem::take(&mut snapshot.tools);
    let text = String::from_utf8(snapshot.with_tools(tools_now).to_bytes()).expect("快照是 UTF-8");
    // 循环的标签（`<loop-round>` 的收尾这些）当时也写的旧名字。
    assert!(
        text.contains(SCHEDULE_NEXT),
        "快照里循环的标签提到工具名：{text}"
    );
    let old = text.replace(SCHEDULE_NEXT, "schedule_wakeup");
    assert!(!old.contains("\"package\""), "老快照不记包：{old}");
    Snapshot::from_bytes(old.as_bytes()).expect("老快照读得懂");
    let old_hash = blobs.put(old.as_bytes()).expect("存得进");
    let dir = home.root.session_dir(&session);
    let (segment, content) = std::fs::read_dir(&dir)
        .expect("读得了")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter_map(|path| Some((path.clone(), std::fs::read_to_string(&path).ok()?)))
        .find(|(_, content)| content.contains(&created.policy.to_string()))
        .expect("找得到造会话那一段");
    std::fs::write(
        &segment,
        content.replacen(&created.policy.to_string(), &old_hash.to_string(), 1),
    )
    .expect("写得进");

    let second = Script::new([Play::Says("好。")]);
    let handle = home.load_with(&session, &second, &tools).await;
    assert!(handle.loops(), "冻着旧名字的照样认 /loop");
    let mut pushes = watch(&handle).await;
    ask(&handle, "m2", say("还在吗")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let names = sent(&second, 0);
    assert!(names.iter().any(|name| name == SCHEDULE_NEXT), "{names:?}");
    assert!(
        !names.iter().any(|name| name == "schedule_wakeup"),
        "{names:?}"
    );
    // 回合开始换上的快照：工具面、循环的标签都是新名字（核心的字照现在的资源重拼）。
    let swapped = home
        .log(&session)
        .into_iter()
        .find_map(|event| match event.body {
            Body::PolicyChanged(changed) => changed.policy,
            _ => None,
        })
        .expect("回合开始换了快照");
    let now = String::from_utf8(blobs.get(&swapped).expect("新快照在 blob 里")).expect("UTF-8");
    assert!(!now.contains("schedule_wakeup"), "{now}");
    let now = Snapshot::from_bytes(now.as_bytes()).expect("读得懂");
    let loops = now.core.schedule.expect("有循环的标签");
    assert!(
        loops.loop_close.contains(SCHEDULE_NEXT),
        "{}",
        loops.loop_close
    );
}
