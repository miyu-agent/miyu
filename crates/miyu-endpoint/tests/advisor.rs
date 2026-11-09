//! 问顾问，真核心走一遍（设计 `69-旗舰顾问.md`，施工 ADV-1、ADV-2，`docs/blueprint/tools/advisor.md`）：她在主会话里调
//! `advisor`，会话表造出顾问会话（无工具、无人格、钉 `@flagship`、system 是 `core/advisor/system.txt`），问题作为主会话发来的
//! 话开它的一轮，替身模型答完，回答原文当工具结果交回。旗舰池里没有可用模型的不造、不发；`continue` 接着最近的那一个，
//! 不写的新开。

use std::sync::Arc;

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event, ToolStatus};
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::{By, Session};
use miyu_kernel::request::{Message, Request};
use miyu_models::settings::{ModelSettings, PoolSettings, ProviderSettings};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::{Client, Home, TOKEN, default_resources};

/// 一家假的供应商、旗舰池里放它的一个模型：请求由替身模型答，配置只用来认池里有没有可用的。
const FLAGSHIP: &str = "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:9/v1\"\n\n[pools.flagship]\nmodels = [\"a/big\"]\n";

/// 起核心（基础系统的工具、配置写成 `config`），握手，在工作区里造一个主会话。
async fn started(home: &Home, script: &Script, config: &str) -> (Client, String) {
    home.write("etc/config.toml", config);
    let tools =
        Catalog::new(miyu_basesystem::tools(&default_resources()).expect("测试的数据合写法"))
            .expect("测试的数据合写法");
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        None,
        [
            miyu_endpoint::settings::PermissionSettings::ITEMS,
            ProviderSettings::ITEMS,
            ModelSettings::ITEMS,
            PoolSettings::ITEMS,
            miyu_models::settings::FIXED_POOL_ITEMS,
        ]
        .concat(),
        miyu_endpoint::config::Environment::of(&[]),
    );
    let core = home
        .core_full(script, tools, None, TOKEN)
        .with_config(config);
    let mut client = Client::connect(Arc::new(core));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    (client, parent)
}

/// 她调一次 `advisor`，参数是 `args`。
fn asks(args: serde_json::Value) -> Play {
    Play::calls(&[("advisor", &args.to_string())])
}

/// 主会话日志里每次调用的结果：状态和那一段字，照先后。
fn results(log: &[Event]) -> Vec<(ToolStatus, String)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some((
                result.status.clone(),
                result
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        Block::Text(Text { text }) => Some(text.as_str()),
                        _ => None,
                    })
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

/// 这个父会话的顾问会话：数据根里第一条带着它、`origin` 是 `advisor` 的，照造的先后。
fn advisors(home: &Home, parent: &str) -> Vec<String> {
    let mut found: Vec<(String, String)> = home
        .root
        .sessions()
        .expect("测试的数据合写法")
        .into_iter()
        .filter_map(|id| {
            let log = home.log(id.as_str());
            match &log.first()?.body {
                Body::SessionCreated(created)
                    if created.parent.as_ref().map(SessionId::as_str) == Some(parent)
                        && created.origin.as_deref() == Some("advisor") =>
                {
                    Some((log[0].at.to_string(), id.to_string()))
                }
                _ => None,
            }
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, id)| id).collect()
}

/// 顾问那一份 system：资源原文去掉末尾的空白。
fn advisor_system() -> String {
    let text = std::fs::read_to_string(default_resources().join("core/advisor/system.txt"))
        .expect("测试的数据合写法");
    text.trim_end().to_string()
}

/// 替身模型收到的、system 是顾问那一份的请求，照先后。
fn advisor_requests(script: &Script) -> Vec<Request> {
    let system = advisor_system();
    script
        .requests()
        .into_iter()
        .map(|(_, request)| request)
        .filter(|request| request.system == system)
        .collect()
}

/// 一次请求里每条消息的字，照先后：人这边的、她的回答。
fn said_in(request: &Request) -> Vec<String> {
    request
        .messages
        .iter()
        .map(|message| {
            let blocks = match message {
                Message::User { blocks, .. } | Message::Assistant { blocks, .. } => blocks,
                _ => return String::new(),
            };
            blocks
                .iter()
                .filter_map(|block| match block {
                    Block::Text(Text { text }) => Some(text.as_str()),
                    _ => None,
                })
                .collect()
        })
        .collect()
}

#[tokio::test]
async fn without_a_flagship_model_nothing_is_asked() {
    let home = Home::new();
    let script = Script::new([
        asks(json!({"question": "Which lock first?"})),
        Play::Says("好。"),
    ]);
    let (mut client, parent) = started(&home, &script, "").await;
    client.say("c2", &parent, "问问顾问").await;
    home.until_turns(&parent, 1).await;

    assert_eq!(
        results(&home.log(&parent)),
        [(
            ToolStatus::Error,
            "No model is set in the flagship pool.\n".to_string()
        )]
    );
    assert!(advisor_requests(&script).is_empty(), "没发请求");
    assert!(advisors(&home, &parent).is_empty(), "没造顾问会话");
    assert_eq!(script.requests().len(), 2, "只有主会话的两次");
}

#[tokio::test]
async fn the_question_goes_to_a_bare_flagship_session_and_the_answer_comes_back() {
    let home = Home::new();
    let question = "Two threads deadlock on A and B.\nWhich lock should go first?";
    let answer = "Take A before B everywhere.";
    let script = Script::new([
        asks(json!({"question": question})),
        Play::Says(answer),
        Play::Says("好。"),
    ]);
    let (mut client, parent) = started(&home, &script, FLAGSHIP).await;
    client.say("c2", &parent, "问问顾问").await;
    home.until_turns(&parent, 1).await;

    // 回答原文当工具结果交回。
    assert_eq!(
        results(&home.log(&parent)),
        [(ToolStatus::Ok, answer.to_string())]
    );
    // 顾问那一次请求：system 是顾问那一份，没有工具，问题是唯一的一条人这边的消息。
    let [asked] = advisor_requests(&script).try_into().expect("问过一次顾问");
    assert!(asked.tools.is_empty(), "顾问没有工具");
    assert_eq!(said_in(&asked), [question.to_string()]);
    // 顾问会话：主会话造的子会话，钉 `@flagship`，无人格、无工具；问题记成主会话发的。
    let [child] = advisors(&home, &parent).try_into().expect("一个顾问会话");
    let log = home.log(&child);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话");
    };
    let from_parent = By::Session(Session {
        id: SessionId::parse(&parent).expect("测试的数据合写法"),
    });
    assert_eq!(log[0].by, from_parent);
    assert_eq!(created.depth, Some(1));
    assert_eq!(created.model.as_deref(), Some("@flagship"));
    assert_eq!(created.persona, None);
    assert!(created.no_tools);
    let Body::MessageUser(message) = &log[1].body else {
        panic!("第 2 条应该是问题：{:?}", log[1].body);
    };
    assert_eq!(log[1].by, from_parent);
    assert_eq!(
        message.blocks,
        [Block::Text(Text {
            text: question.to_string()
        })]
    );
}

/// 主会话问三次：第一次新开，第二次写了 `continue` 接着它（看得到第一次的问答），第三次不写又新开（只看得到自己的问题）。
#[tokio::test]
async fn continue_goes_on_with_the_last_advisor_and_otherwise_a_new_one_opens() {
    let home = Home::new();
    let script = Script::new([
        asks(json!({"question": "Q1"})),
        Play::Says("A1"),
        Play::Says("好。"),
        asks(json!({"question": "Q2", "continue": true})),
        Play::Says("A2"),
        Play::Says("好。"),
        asks(json!({"question": "Q3"})),
        Play::Says("A3"),
        Play::Says("好。"),
    ]);
    let (mut client, parent) = started(&home, &script, FLAGSHIP).await;
    for (turn, id) in ["c2", "c3", "c4"].into_iter().enumerate() {
        client.say(id, &parent, "问问顾问").await;
        home.until_turns(&parent, turn + 1).await;
    }

    let answers: Vec<String> = results(&home.log(&parent))
        .into_iter()
        .map(|(_, text)| text)
        .collect();
    assert_eq!(answers, ["A1", "A2", "A3"]);
    let asked: Vec<Vec<String>> = advisor_requests(&script).iter().map(said_in).collect();
    assert_eq!(
        asked,
        [
            vec!["Q1".to_string()],
            vec!["Q1".to_string(), "A1".to_string(), "Q2".to_string()],
            vec!["Q3".to_string()],
        ]
    );
    assert_eq!(advisors(&home, &parent).len(), 2, "接着说的没另开");
}

/// 一个顾问会话都没有时写 `continue` 照新开（设计 69 第一节第 5 条）。
#[tokio::test]
async fn continue_without_an_earlier_advisor_opens_one() {
    let home = Home::new();
    let script = Script::new([
        asks(json!({"question": "Q1", "continue": true})),
        Play::Says("A1"),
        Play::Says("好。"),
    ]);
    let (mut client, parent) = started(&home, &script, FLAGSHIP).await;
    client.say("c2", &parent, "问问顾问").await;
    home.until_turns(&parent, 1).await;
    assert_eq!(
        results(&home.log(&parent)),
        [(ToolStatus::Ok, "A1".to_string())]
    );
    assert_eq!(advisors(&home, &parent).len(), 1);
}

/// 一步里并着调两次、都写 `continue`：同一个顾问会话一次只答一个问题，一个接着上一次的，另一个新开；两个都答得回来，
/// 不会一个排进另一个正在答的那一轮、一直等不到自己的那一轮。
#[tokio::test]
async fn two_continues_at_once_do_not_wait_on_one_advisor() {
    let home = Home::new();
    let both = [
        (
            "advisor",
            json!({"question": "Q2", "continue": true}).to_string(),
        ),
        (
            "advisor",
            json!({"question": "Q3", "continue": true}).to_string(),
        ),
    ];
    let calls: Vec<(&str, &str)> = both
        .iter()
        .map(|(name, args)| (*name, args.as_str()))
        .collect();
    let script = Script::new([
        asks(json!({"question": "Q1"})),
        Play::Says("A1"),
        Play::calls(&calls),
        Play::Says("A"),
        Play::Says("A"),
        Play::Says("好。"),
    ]);
    let (mut client, parent) = started(&home, &script, FLAGSHIP).await;
    client.say("c2", &parent, "问问顾问").await;
    home.until_turns(&parent, 1).await;
    let answers: Vec<String> = results(&home.log(&parent))
        .into_iter()
        .map(|(_, text)| text)
        .collect();
    assert_eq!(answers, ["A1", "A", "A"]);
    assert_eq!(advisors(&home, &parent).len(), 2, "第二个另开了一个");
}
