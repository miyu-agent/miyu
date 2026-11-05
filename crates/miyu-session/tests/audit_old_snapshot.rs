//! 老会话的工具面字节不变的复核（施工 XG-2，`docs/designs/64-工具职能.md` 第七节；测试会话 e0 的对抗式复核）：快照里的工具面
//! 只多了 `role` 一格，没写的不写，所以把新会话快照里的 `"role":"read_file"` 拿掉，就是 XG-2 以前造的那份字节。把会话的
//! `session.created` 指到这份老快照上再载入：请求的 `tools` 和 `system`（技能那一块照名字 `read` 认的退路）和老快照之前那一轮
//! 逐字节一样。

use miyu_kernel::event::Body;
use miyu_policy::Snapshot;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;

use crate::support::jobs::base_system;
use crate::support::*;

/// 在数据根里放一个技能：工具面上有读文件的才列进 system。
fn skill(home: &Home) {
    let file = home.root.skills().join("notes").join("SKILL.md");
    std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了");
    std::fs::write(
        &file,
        "---\nname: notes\ndescription: Take notes.\n---\n\n照这样记。\n",
    )
    .expect("写得进");
}

#[tokio::test]
async fn a_snapshot_from_before_roles_asks_with_the_same_tools_and_system() {
    let home = Home::new();
    skill(&home);
    let tools = base_system();
    let first = Script::new([Play::Says("好。")]);
    let handle = home
        .create_full(&first, &tools, Opening::default(), Lines::default())
        .await;
    let session = handle.id().clone();
    let mut pushes = watch(&handle).await;
    ask(&handle, "m1", say("在吗")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    drop(pushes);
    stop(&handle).await;
    let before = first.requests()[0].1.clone();
    assert!(
        before.system.contains("Skills you can use"),
        "新会话照职能认读文件的，列了技能：{}",
        before.system
    );

    // 拿掉职能那一格，就是 XG-2 以前的快照。
    let blobs = Blobs::new(home.root.blobs());
    let log = home.log(handle.id());
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话：{:?}", log[0]);
    };
    let bytes = blobs.get(&created.policy).expect("快照在 blob 里");
    let text = String::from_utf8(bytes.clone()).expect("快照是 UTF-8");
    let role = r#","role":"read_file""#;
    assert_eq!(text.matches(role).count(), 1, "只有 read 写了职能：{text}");
    let old = text.replacen(role, "", 1);
    let old_hash = blobs.put(old.as_bytes()).expect("存得进");
    let snapshot = Snapshot::from_bytes(old.as_bytes()).expect("老快照读得懂");
    assert!(
        snapshot.tools.iter().all(|tool| tool.role.is_none()),
        "老快照一件都没写职能"
    );
    assert_eq!(snapshot.hash(), old_hash, "老快照的哈希照字节算");

    // 日志里造会话那一条指到老快照上。
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
    let mut pushes = watch(&handle).await;
    ask(&handle, "m2", say("还在吗")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let after = second.requests()[0].1.clone();
    assert_eq!(
        serde_json::to_string(&after.tools).expect("写得成"),
        serde_json::to_string(&before.tools).expect("写得成"),
        "老快照发给模型的工具面逐字节一样"
    );
    assert_eq!(after.system, before.system, "system 照快照里定的");
    // 技能那一块造会话时定在 system 里，以后每回合照快照里的工具面再找一遍，和定的不一样就在这一轮另交一块「技能变了」
    // （`skills/watch.rs` 的 `tail`）。老快照照名字 `read` 认得出读文件的，找出来的一样，不另交。
    let changed: Vec<_> = home
        .log(&session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == "skills" => Some(fact.text),
            _ => None,
        })
        .collect();
    assert!(
        changed.is_empty(),
        "老快照的技能那一块没变，不另交：{changed:?}"
    );
}
