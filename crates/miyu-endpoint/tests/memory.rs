//! 回合索引接在核心上（施工 R-2 上，`docs/blueprint/memory.md`「怎么走」第一条第 4、7 款）：真核心造的主会话说过的一轮进
//! 软件工程师的回合库；删会话以后拿掉。

use serde_json::json;

use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 经 `memory.turns` 在软件工程师的回合库里搜 `words`，交回找到的那几轮是哪个会话的（施工 R-33）。核心搜以前先等回合索引的
/// 后台写完（`Memory::settle`）：不另开连接直接读库——核心头一次开库时在一张张建表，另开的连接这时查会撞上 `SchemaChanged`
/// （main 的 CI 红过两次）。
async fn found(client: &mut Client, id: &str, words: &str) -> Vec<String> {
    let reply = client
        .call(
            id,
            "memory.turns",
            json!({"persona": "engineer", "query": words}),
        )
        .await;
    reply["result"]["turns"]
        .as_array()
        .unwrap_or_else(|| panic!("有 turns：{reply}"))
        .iter()
        .map(|turn| turn["session"].as_str().expect("有会话").to_string())
        .collect()
}

#[tokio::test]
async fn a_turn_said_through_the_core_is_indexed_and_goes_with_the_session() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Says("好，记住你用 N 卡。")])));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let session = client.create_as("c1", &work, "engineer").await;
    client.say("c2", &session, "我的显卡是 N 卡").await;
    home.until_turns(&session, 1).await;
    // 日志先落盘、这一轮后交给回合索引的后台：看到这一轮结束时可能还没交过去，交过去以后 `memory.turns` 等它写完再搜。最多
    // 六十秒（机器忙时慢，只在出错时等满）。
    let mut sessions = found(&mut client, "t0", "显卡").await;
    for n in 1..1200 {
        if !sessions.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        sessions = found(&mut client, &format!("t{n}"), "显卡").await;
    }
    assert_eq!(sessions, vec![session.clone()]);

    let reply = client
        .call("c3", "session.delete", json!({"session": session}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert!(
        found(&mut client, "c4", "显卡").await.is_empty(),
        "删会话拿掉"
    );
    // 上面那一次搜等过回合索引的后台写完：库早建好了，这时另开只读的连接看墓碑。
    let path = home.root.index().join("recall").join("turns-engineer.db");
    let db =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("回合库在");
    let buried: i64 = db
        .query_row(
            "SELECT count(*) FROM buried WHERE key = ?1",
            [format!("{session}/")],
            |row| row.get(0),
        )
        .expect("查得了");
    assert_eq!(buried, 1, "整个会话埋了墓碑：记忆的出处在它里面的都算死了");
}

/// 常驻的摘要接在核心上（施工 R-4 上，`memory.md` 第三条第 5 款）：核心起来时读好外壳的字交给记忆；人经协议记了一条，新会话
/// 第一轮的请求里有那一块。
#[tokio::test]
async fn a_new_session_starts_with_what_was_remembered() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut client = crate::support::memories::connected(&home, &script).await;
    let reply = client
        .call(
            "c1",
            "memory.remember",
            json!({"class": "user", "text": "用户养了一只猫"}),
        )
        .await;
    assert_eq!(reply["result"], json!({"id": "m1"}), "{reply}");
    let work = home.work.to_string_lossy().into_owned();
    let session = client.create_as("c2", &work, "engineer").await;
    client.say("c3", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    let request = format!("{:?}", script.requests()[0].1);
    assert!(
        request.contains(r"<memories>\nm1 user ")
            && request.contains(r": 用户养了一只猫\n</memories>"),
        "{request}"
    );
}
