//! 分叉出来的支线也记得（施工 R-13 三补，`docs/blueprint/memory.md` 第一条第 1 款、「范围」）：群里第二个人 @ 她时分叉出的
//! 支线照父会话的记忆范围（子代理照旧 `off`）：工具面上有记忆的三件、记得下；支线自己的回合照支线的编号进回合库，父会话
//! 的回合不在支线名下再记一遍。真记忆日志、真回合库，执行器替身照剧本调工具，数据根在临时目录。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event, VenueMessage};
use miyu_kernel::id::{CommandId, ExternalId, VenueId};
use miyu_kernel::origin::{By, External, Role};
use miyu_kernel::request::Request;
use miyu_kernel::session::Command;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Fork, Handle, new_id};
use miyu_store::recall::Room;
use miyu_tool::Catalog;

use crate::support::*;

/// 基础系统加记忆的三件，照包登记，和核心里一样。
fn catalog(home: &Home) -> Catalog {
    let resources = home.resources.path();
    Catalog::in_packages(vec![
        (
            miyu_tool::BASESYSTEM,
            miyu_basesystem::tools(resources).expect("读得出"),
        ),
        (
            miyu_memory::PACKAGE,
            miyu_memory::tools(resources).expect("读得出"),
        ),
    ])
    .expect("合写法")
}

/// 说一句，等这一轮结束。
async fn chat(handle: &Handle, id: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, id, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

fn names(request: &Request) -> Vec<&str> {
    request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect()
}

/// 日志里最后一次调用的结果。
fn last_result(log: &[Event]) -> String {
    log.iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(format!("{:?}", result.blocks)),
            _ => None,
        })
        .expect("有一次调用")
}

#[tokio::test]
async fn a_lane_remembers_like_its_main_session_and_its_turns_go_under_its_own_name() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("早上好。"),
        Play::calls(&[("remember", r#"{"class":"user","text":"团子爱吃鸡胸肉"}"#)]),
        Play::Says("记住了。"),
    ]);
    let tools = catalog(&home);
    let parent = home.create_with(&script, &tools).await;
    chat(&parent, "c1", "早").await;
    let lane = new_id(now());
    miyu_session::fork(Fork {
        root: &home.root,
        owner: &alice_account(),
        parent: parent.id(),
        id: lane.clone(),
        cause: CommandId::parse("f1").expect("合写法"),
        by: alice(),
        at: now(),
        index: None,
        seal_after: None,
    })
    .await
    .expect("分叉得了");
    let branch = home.load_with(&lane, &script, &tools).await;
    chat(&branch, "c2", "团子爱吃什么").await;
    let requests = script.requests();
    assert!(
        names(&requests[1].1).contains(&"remember"),
        "支线照父会话的范围，工具面上有记忆的三件：{:?}",
        names(&requests[1].1)
    );
    // 支线的请求前缀和主线逐字节相同（C-8 的约定，缓存靠它）：system、工具面一样，消息照主线发过的接着往下。
    let (main, branch_request) = (&requests[0].1, &requests[1].1);
    assert_eq!(branch_request.system, main.system);
    assert_eq!(branch_request.tools, main.tools);
    assert_eq!(
        branch_request.messages[..main.messages.len()],
        main.messages[..],
        "支线的消息照主线发过的开头"
    );
    assert!(
        last_result(&home.log(&lane)).contains("Saved as m1."),
        "支线里记得下：{}",
        last_result(&home.log(&lane))
    );
    stop(&branch).await;
    stop(&parent).await;
    home.memory.settle();
    let (index, _) = home
        .recall
        .turns(&Room::persona(&alice_account(), "engineer"));
    let keys = index.keys().expect("列得出");
    let mine: Vec<&String> = keys
        .iter()
        .filter(|key| key.starts_with(&format!("{lane}/")))
        .collect();
    assert_eq!(
        mine.len(),
        1,
        "支线自己的那一轮进回合库，父会话的不再记一遍：{keys:?}"
    );
    assert_eq!(
        keys.iter()
            .filter(|key| key.starts_with(&format!("{}/", parent.id())))
            .count(),
        1,
        "父会话那一轮照旧：{keys:?}"
    );
}

/// 群里的小林（`qq:20017`）。
fn member() -> By {
    By::External(External {
        venue: VenueId::parse("qq:group:1").expect("合写法"),
        id: ExternalId::parse("qq:20017").expect("合写法"),
        account: None,
        role: Some(Role::Member),
    })
}

/// 群里的主线旁听记下小林的一句「团子吃什么」，分叉一条支线，支线照这一句开一轮、她回一句；都停下。交回支线的编号。
async fn heard_then_answered_in_a_lane(home: &Home) -> miyu_kernel::id::SessionId {
    let script = Script::new([Play::Says("猫粮加一点鸡胸肉。")]);
    let tools = catalog(home);
    let lines = Lines {
        venue: VenueId::parse("qq:group:1").expect("合写法"),
        group: true,
        ..Lines::default()
    };
    let parent = home
        .create_full(&script, &tools, Opening::default(), lines)
        .await;
    let heard = Command::Send {
        blocks: vec![Block::Text(Text {
            text: "团子吃什么".to_string(),
        })],
        urgent: false,
        venue: Some(VenueMessage {
            msg: "8800".to_string(),
            name: Some("小林".to_string()),
            ambient: true,
            ..VenueMessage::default()
        }),
    };
    within("回应", parent.command(id("h1"), member(), heard))
        .await
        .expect("会话在跑");
    let line = home
        .log(parent.id())
        .iter()
        .find(|event| matches!(event.body, Body::MessageUser(_)))
        .map(|event| event.seq)
        .expect("旁听记下了");
    let lane = new_id(now());
    miyu_session::fork(Fork {
        root: &home.root,
        owner: &alice_account(),
        parent: parent.id(),
        id: lane.clone(),
        cause: CommandId::parse("f1").expect("合写法"),
        by: alice(),
        at: now(),
        index: None,
        seal_after: None,
    })
    .await
    .expect("分叉得了");
    let branch = home.load_with(&lane, &script, &tools).await;
    let mut pushes = watch(&branch).await;
    let respond = Command::Respond {
        to: vec![line],
        facts: Vec::new(),
    };
    ask(&branch, "r1", respond).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    stop(&branch).await;
    stop(&parent).await;
    lane
}

/// 回合库里搜「团子」：键和字。
fn tuanzi(memory: &miyu_session::Memory) -> Vec<(String, String)> {
    memory.settle();
    let (index, _) = memory
        .turns
        .turns(&Room::persona(&alice_account(), "engineer"));
    index
        .search("团子", 10)
        .expect("搜得了")
        .into_iter()
        .map(|hit| (hit.key, hit.text))
        .collect()
}

/// 不在 Windows 上跑：这个测试里载入支线的那一份记忆还开着回合库，Windows 上开着的文件删不掉，新的一份重建不了（真用时重建
/// 是核心起来那一刻，没有别人开着）。CI 上 Windows 报「等不到补齐」查出来的。
#[cfg(not(windows))]
#[tokio::test]
async fn a_rebuilt_index_fills_a_lane_from_its_base_too() {
    // 回合库重建以后补齐旧会话（`memory.md` 第一条第 9 款）：支线照载入时一样读底子，那一段带着底子里那句人话。
    let home = Home::new();
    let lane = heard_then_answered_in_a_lane(&home).await;
    let path = home
        .root
        .index(&alice_account())
        .join("recall/turns-engineer.db");
    std::fs::write(&path, "这不是一个库").expect("写得进");
    let memory = miyu_session::Memory::new(&home.root, None);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let found = tuanzi(&memory);
        if !found.is_empty() {
            assert_eq!(found.len(), 1, "{found:?}");
            assert!(found[0].0.starts_with(&format!("{lane}/")), "{found:?}");
            assert_eq!(found[0].1, "团子吃什么\n\n猫粮加一点鸡胸肉。");
            break;
        }
        assert!(std::time::Instant::now() < deadline, "等不到补齐");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn a_lane_turn_opened_by_a_line_heard_before_the_fork_keeps_that_line() {
    // 支线那一轮的触发在底子里（分叉点以前主线旁听记下的）：回合库那一段带着这句人话，只在支线名下记一次。
    let home = Home::new();
    let lane = heard_then_answered_in_a_lane(&home).await;
    let found = tuanzi(&home.memory);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].0.starts_with(&format!("{lane}/")), "{found:?}");
    assert_eq!(found[0].1, "团子吃什么\n\n猫粮加一点鸡胸肉。");
}
