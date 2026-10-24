//! 界面块的内容（施工 X-15 中，`docs/blueprint/widgets.md`「程序发给核心」「核心告诉程序」「头」）：真核心拉起测试用的扩展。头订阅
//! 以后程序收到 `widget.watch`、推的内容转到头（照语言挑）；头不看了程序收到 `widget.unwatch`、跟着会话的内容扔掉、全局的留着；
//! 同一块推得再快，头那边每 250 毫秒最多一次、最后一份不丢；写法不对的拒、原来的不动；程序关掉推 `null`。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

/// 包 `wx`：要 `widgets` 能力，侧边栏一块（跟着会话）、底栏一块、整页一块。
fn manifest(program: &str, args: &[String]) -> String {
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Weather\" }}\n\n[command]\nname = \"wx\"\nprogram = \"{program}\"\nabout = {{ en = \"W\" }}\n\n[process]\nargs = [{}]\nstart = \"manual\"\ncapabilities = [\"widgets\"]\n\n[[widget]]\nid = \"today\"\nplace = \"sidebar\"\ntitle = {{ en = \"Weather\", zh = \"天气\" }}\nscope = \"session\"\n\n[[widget]]\nid = \"temp\"\nplace = \"status\"\n\n[[widget]]\nid = \"week\"\nplace = \"page\"\ntitle = {{ en = \"This week\" }}\n",
        args.join(", ")
    )
}

/// 真核心、开好 `wx`（批了 `widgets`、登记了方法 `burst`、`quit`），先做完 `extra` 那几步再 `serve`；一个本机的会话。交回记的文件。
async fn started(
    home: &Home,
    program: &Program,
    extra: &[&str],
) -> (Arc<miyu_endpoint::Core>, Client, String, std::path::PathBuf) {
    started_with(home, program, extra, quick()).await
}

/// 同 [`started`]，退避照 `timing`。
async fn started_with(
    home: &Home,
    program: &Program,
    extra: &[&str],
    timing: miyu_endpoint::extensions::Timing,
) -> (Arc<miyu_endpoint::Core>, Client, String, std::path::PathBuf) {
    let (path, step) = record(home, "wx");
    let mut all = vec![
        step.as_str(),
        "hello",
        r#"ask:package.methods:{"methods":[{"name":"burst"},{"name":"quit"}]}"#,
    ];
    all.extend_from_slice(extra);
    all.push("serve");
    home.write(
        "system/packages/wx/package.toml",
        &manifest(&program.name(), &steps(&all)),
    );
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(timing),
    );
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let enabled = client
        .call(
            "e1",
            "extension.enable",
            json!({"package": "wx", "approve": ["widgets"]}),
        )
        .await;
    assert!(enabled["result"].is_object(), "{enabled}");
    let wanted = 3 + extra.len();
    until("程序做完前面几步", || {
        read(&path).lines().count() >= wanted
    })
    .await;
    let session = client.create("c1", "~").await;
    (core, client, session, path)
}

async fn subscribe(client: &mut Client, id: &str, params: Value) -> Value {
    let mut params = params;
    params["stream"] = json!("widgets");
    // 等这一条的回应：上一个订阅推的 `widget.changed` 可能先到（Windows CI 上撞见过，施工 X-15 修）。
    let request = json!({"jsonrpc": "2.0", "id": id, "method": "subscribe", "params": params});
    client.line(&request.to_string()).await;
    client.until_reply(id).await.1
}

/// 回应里编号是 `id` 的那一块。
fn widget(reply: &Value, id: &str) -> Value {
    reply["result"]["widgets"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|one| one["id"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("没列出 {id}：{reply}"))
}

/// 读推送，读到块 `id` 的 `widget.changed`、满足 `wanted` 的那一次。
async fn until_changed(client: &mut Client, id: &str, wanted: impl Fn(&Value) -> bool) -> Value {
    loop {
        let next = client.next().await.expect("没断开");
        if next["method"] == "widget.changed"
            && next["params"]["id"] == id
            && wanted(&next["params"]["widget"])
        {
            return next["params"]["widget"].clone();
        }
    }
}

/// 读推送，读到 `ids` 每一块都有了内容：交回各自最后的那一份，照 `ids` 的先后。
async fn until_filled(client: &mut Client, ids: &[&str]) -> Vec<Value> {
    let mut got: Vec<Option<Value>> = vec![None; ids.len()];
    while got.iter().any(Option::is_none) {
        let next = client.next().await.expect("没断开");
        let widget = &next["params"]["widget"];
        if next["method"] != "widget.changed" || !widget["content"].is_array() {
            continue;
        }
        if let Some(at) = ids.iter().position(|id| next["params"]["id"] == *id) {
            got[at] = Some(widget.clone());
        }
    }
    got.into_iter().flatten().collect()
}

fn text(widget: &Value) -> &str {
    widget["content"][0]["text"].as_str().unwrap_or_default()
}

#[tokio::test]
async fn a_look_makes_the_program_push_and_the_head_sees_it() {
    let home = Home::new();
    let program = Program::new();
    let (_core, mut client, session, path) = started(&home, &program, &[]).await;
    let reply = subscribe(
        &mut client,
        "w1",
        json!({"session": session, "showing": ["sidebar", "status"]}),
    )
    .await;
    let today = widget(&reply, "today");
    assert_eq!(
        (
            &today["title"],
            &today["source"],
            &today["scope"],
            &today["running"]
        ),
        (
            &json!("天气"),
            &json!("Weather"),
            &json!("session"),
            &json!(true)
        ),
        "{reply}"
    );
    assert_eq!(today["content"], Value::Null, "程序还没推");
    assert!(
        widget(&reply, "week").get("content").is_none(),
        "没开的整页不带内容"
    );
    let filled = until_filled(&mut client, &["today", "temp"]).await;
    assert_eq!(text(&filled[0]), format!("watching today {session}"));
    assert_eq!(text(&filled[1]), "watching");
    let told = read(&path);
    assert!(
        told.contains(&format!(
            r#""method":"widget.watch","params":{{"session":"{session}","widget":"today"}}"#
        )),
        "{told}"
    );
    assert!(
        told.contains(r#""method":"widget.watch","params":{"widget":"temp"}"#),
        "{told}"
    );
    assert!(
        !told.contains(r#""widget":"week"}"#),
        "没开的整页不叫它：{told}"
    );
}

#[tokio::test]
async fn leaving_tells_the_program_and_drops_what_followed_the_session() {
    let home = Home::new();
    let program = Program::new();
    let (_core, mut client, session, path) = started(&home, &program, &[]).await;
    subscribe(
        &mut client,
        "w1",
        json!({"session": session, "showing": ["sidebar", "status"]}),
    )
    .await;
    until_filled(&mut client, &["today", "temp"]).await;
    // 填好以后还可能推来别的 `widget.changed`：等这一条的回应。
    let request = json!({"jsonrpc": "2.0", "id": "u1", "method": "unsubscribe", "params": {"stream": "widgets"}});
    client.line(&request.to_string()).await;
    let gone = client.until_reply("u1").await.1;
    assert_eq!(gone["result"], json!({}), "{gone}");
    until("程序收到不看了", || {
        read(&path).matches(r#""method":"widget.unwatch""#).count() == 2
    })
    .await;
    let again = subscribe(
        &mut client,
        "w2",
        json!({"session": session, "showing": ["sidebar", "status"]}),
    )
    .await;
    assert_eq!(
        widget(&again, "today")["content"],
        Value::Null,
        "跟着会话的扔掉了"
    );
    assert_eq!(text(&widget(&again, "temp")), "watching", "全局的留着");
    // 没带会话的不列跟着会话的块。
    let home_screen = subscribe(&mut client, "w3", json!({"showing": ["home", "status"]})).await;
    let ids: Vec<&Value> = home_screen["result"]["widgets"]
        .as_array()
        .unwrap_or_else(|| panic!("{home_screen}"))
        .iter()
        .map(|one| &one["id"])
        .collect();
    assert_eq!(ids, [&json!("temp"), &json!("week")]);
}

#[tokio::test]
async fn a_burst_reaches_the_head_at_most_every_quarter_second_and_ends_on_the_last() {
    let home = Home::new();
    let program = Program::new();
    let (_core, mut client, _session, _path) = started(&home, &program, &[]).await;
    subscribe(&mut client, "w1", json!({"showing": ["status"]})).await;
    until_changed(&mut client, "temp", |one| one["content"].is_array()).await;
    let request = json!({"jsonrpc": "2.0", "id": "b1", "method": "package.call", "params": {"package": "wx", "method": "burst"}});
    client.line(&request.to_string()).await;
    let (mut pushed, reply) = client.until_reply("b1").await;
    assert_eq!(reply["result"]["answered"], "burst", "{reply}");
    while let Some(next) = client.next_within(Duration::from_millis(800)).await {
        pushed.push(next);
    }
    let texts: Vec<String> = pushed
        .iter()
        .filter(|one| one["method"] == "widget.changed" && one["params"]["id"] == "temp")
        .map(|one| text(&one["params"]["widget"]).to_string())
        .collect();
    assert_eq!(texts.last().map(String::as_str), Some("n=9"), "{texts:?}");
    assert!(texts.len() <= 3, "十次只推了几次：{texts:?}");
}

#[tokio::test]
async fn wrong_content_is_refused_and_the_old_one_stays() {
    let home = Home::new();
    let program = Program::new();
    let extra = [
        r#"ask:widget.set:{"widget":"temp","content":[{"type":"text","text":"23°C"}]}"#,
        r#"ask:widget.set:{"widget":"week","content":[{"type":"table"}]}"#,
        r#"ask:widget.set:{"widget":"temp","content":[{"type":"text","text":"a\u0007b"}]}"#,
        r#"ask:widget.set:{"widget":"nope","content":[]}"#,
        r#"ask:widget.set:{"widget":"today","content":[]}"#,
        r#"ask:widget.set:{"widget":"today","session":"01920000-0000-7000-8000-000000000000","content":[]}"#,
        r#"ask:widget.clear:{"widget":"week"}"#,
        r#"ask:widget.set:{"widget":"today","session":"not a session","content":[]}"#,
    ];
    let (_core, mut client, _session, path) = started(&home, &program, &extra).await;
    let replies: Vec<Value> = read(&path)
        .lines()
        .skip(3)
        .take(extra.len())
        .map(|line| serde_json::from_str(line).expect("回应是 JSON"))
        .collect();
    assert_eq!(replies[0]["result"], json!({"kept": true}), "{replies:?}");
    assert_eq!(reason(&replies[1]), Some("bad_widget_content"));
    assert_eq!(replies[1]["error"]["data"]["path"], "content[0].type");
    assert_eq!(replies[1]["error"]["data"]["problem"], "type");
    assert_eq!(replies[2]["error"]["data"]["problem"], "control_char");
    assert_eq!(replies[2]["error"]["data"]["path"], "content[0].text");
    assert_eq!(reason(&replies[3]), Some("unknown_widget"));
    assert_eq!(replies[3]["error"]["data"]["widget"], "nope");
    assert_eq!(
        reason(&replies[4]),
        Some("bad_params"),
        "跟着会话的没写会话"
    );
    assert_eq!(
        replies[5]["result"],
        json!({"kept": false}),
        "没人看的会话不留"
    );
    assert_eq!(replies[6]["result"], json!({}));
    assert_eq!(reason(&replies[7]), Some("bad_params"), "会话编号写法不对");
    let reply = subscribe(&mut client, "w1", json!({"showing": ["status"]})).await;
    assert_eq!(text(&widget(&reply, "temp")), "23°C", "写错的没动原来的");
    // 头的连接推不了。订阅以后扩展推的 `widget.changed` 可能先到，等这一条的回应（Windows CI 上撞见过，施工 V-14）。
    let request = json!({"jsonrpc": "2.0", "id": "x1", "method": "widget.set", "params": {"widget": "temp", "content": []}});
    client.line(&request.to_string()).await;
    let head = client.until_reply("x1").await.1;
    assert_eq!(reason(&head), Some("not_an_extension"), "{head}");
}

#[tokio::test]
async fn switching_the_program_off_takes_its_widgets_away() {
    let home = Home::new();
    let program = Program::new();
    let (_core, mut client, _session, _path) = started(&home, &program, &[]).await;
    subscribe(&mut client, "w1", json!({"showing": ["status"]})).await;
    until_changed(&mut client, "temp", |one| one["content"].is_array()).await;
    let request = json!({"jsonrpc": "2.0", "id": "d1", "method": "extension.disable", "params": {"package": "wx"}});
    client.line(&request.to_string()).await;
    let gone = until_changed(&mut client, "temp", Value::is_null).await;
    assert_eq!(gone, Value::Null);
}

#[tokio::test]
async fn subscribing_checks_its_params() {
    let home = Home::new();
    let program = Program::new();
    let (_core, mut client, _session, _path) = started(&home, &program, &[]).await;
    for params in [
        json!({"showing": ["page"]}),
        json!({"showing": ["footer"]}),
        json!({"session": "not a session"}),
        json!({"showing": [], "after": 3}),
        json!({"page": {"package": "wx"}}),
    ] {
        let reply = subscribe(&mut client, "w", params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}: {reply}");
    }
}

#[tokio::test]
async fn a_program_that_stops_leaves_nothing_and_the_next_one_is_told_again() {
    let home = Home::new();
    let program = Program::new();
    let mut timing = quick();
    // 退避两秒：这中间没有新内容，清空的那一次不会被限速并掉。
    timing.backoff = Duration::from_secs(2);
    timing.longest = Duration::from_secs(2);
    let (_core, mut client, _session, path) = started_with(&home, &program, &[], timing).await;
    subscribe(&mut client, "w1", json!({"showing": ["status"]})).await;
    until_changed(&mut client, "temp", |one| one["content"].is_array()).await;
    let request = json!({"jsonrpc": "2.0", "id": "q1", "method": "package.call", "params": {"package": "wx", "method": "quit"}});
    client.line(&request.to_string()).await;
    until_changed(&mut client, "temp", |one| one["content"].is_null()).await;
    let back = until_changed(&mut client, "temp", |one| {
        one["content"].is_array() && one["running"] == true
    })
    .await;
    assert_eq!(text(&back), "watching");
    assert_eq!(
        read(&path)
            .matches(r#""method":"widget.watch","params":{"widget":"temp"}"#)
            .count(),
        2,
        "新的程序握了手又说一遍"
    );
}
