//! 运行日志里怎么写一间记忆（施工 LG-1 下（记忆），`docs/blueprint/log.md` 第七节）：不写人格名，写种类 `room_kind` 和
//! 照数据根的盐算的短号 `room_id`。
//!
//! 全局装一个写进内存的订阅者：开回合库在后台线程里。全局的一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。

mod support;

use miyu_kernel::origin::{By, Person};
use miyu_log::{LevelFilter, Memory as Logs};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Filter, Keeper, Listener};
use miyu_store::recall::{LogSalt, Room};

use support::*;

#[tokio::test]
async fn memory_logs_name_the_room_by_kind_and_short_id_only() {
    let logs = Logs::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        logs.clone(),
        LevelFilter::INFO,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("我养了一只猫"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    home.memory.settle();
    // 记忆日志那一边第一次开也记一行（施工 R-3 上）：人列一次就开。
    let room = Room::persona("engineer");
    let keeper = Keeper::new(
        &home.memory,
        room.clone(),
        Listener::Owner(By::Person(Person::default())),
    );
    keeper.list(&Filter::default()).expect("列得出");

    let lines = logs.lines();
    let id = LogSalt::load(&home.root)
        .expect("盐写下了，读得回来")
        .room_id(&room);
    let expected = format!("memory index created room_kind=persona room_id={id}");
    assert!(
        lines.iter().any(|line| line.contains(&expected)),
        "人格那一间开库记一行，短号照数据根的盐算：{lines:#?}"
    );
    let opened = format!("memory log index created room_kind=persona room_id={id}");
    assert!(
        lines.iter().any(|line| line.contains(&opened)),
        "记忆日志那一边同一个短号：{lines:#?}"
    );
    let about_memory: Vec<&String> = lines
        .iter()
        .filter(|line| line.contains("memory"))
        .collect();
    assert!(!about_memory.is_empty());
    for line in about_memory {
        assert!(
            !line.contains("alice") && !line.contains("engineer") && !line.contains(" room="),
            "记忆的日志不写账号、人格名：{line}"
        );
    }
}
