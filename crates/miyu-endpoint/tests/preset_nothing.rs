//! 「无功能」（施工 P-17，`docs/blueprint/presets.md`「出厂的几份」）：出厂一份真预设 `none`，预设菜单里能开关的全关着。项目主人
//! 在无人格、无工具的会话里问出来的八样（关于你、本机路径那一句、harness 版本、环境那一块、权限级别、会话编号、模型、记忆），
//! 选「无功能」的会话一样都没有；以前的无工具（`preset: null`）照旧。

use std::io::Write as _;

use serde_json::{Value, json};

use miyu_kernel::request::Request;
use miyu_policy::shown::Shown;
use miyu_session::testkit::{Play, Script};

use crate::preset_face::snapshot;
use crate::support::*;

/// 写进 alice 个人设置的称呼。
const PROFILE: &str = "[profile]\nname = \"小林\"\n";

/// 请求里的全部字（system 加上消息），照调试的写法：事实在消息里。
fn all_of(request: &Request) -> String {
    format!("{request:?}")
}

/// 核心资源 `core/<name>` 里占位符前面那一截。
fn line(name: &str) -> String {
    let text = std::fs::read_to_string(default_resources().join("core").join(name))
        .expect("读得出核心的那一行");
    text.trim_end().split('{').next().expect("有字").to_string()
}

/// 八样里请求里认得出的那几样（测试的核心没配模型，模型那一块照快照的开关看）：事实、记忆、system 那几行。
fn seen(request: &Request) -> Vec<&'static str> {
    let all = all_of(request);
    let mut seen: Vec<&str> = [
        ("<user>", "profile"),
        ("<env time=", "env"),
        ("cwd=", "cwd"),
        ("<permission level=", "permission"),
        ("<session id=", "session"),
        ("<memories>", "memories"),
    ]
    .into_iter()
    .filter(|(mark, _)| all.contains(mark))
    .map(|(_, name)| name)
    .collect();
    for (name, label) in [
        ("permission-rule.txt", "permission-rule"),
        ("local-paths-rule.txt", "local-paths-rule"),
        ("machine.txt", "machine"),
        ("agent-version.txt", "agent-version"),
    ] {
        if request.system.contains(&line(name)) {
            seen.push(label);
        }
    }
    seen
}

/// 记一条无人格那一间的记忆、用 `params` 开会话说一句，交回会话编号和第一次请求。核心读个人设置里的称呼（「关于你」）和预设
/// 那几项。
async fn opened(home: &Home, params: Value) -> (String, Request) {
    let script = Script::new([Play::Says("嗯。")]);
    let items = [
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_session::switches::ITEMS,
        miyu_session::packages_off::ITEMS,
    ]
    .concat();
    let mut client = Client::connect(home.core_with_items(&script, None, &[], &items));
    client.hello().await;
    let reply = client
        .call(
            "m",
            "memory.remember",
            json!({"persona": "_none", "class": "user", "text": "用户养了一只猫"}),
        )
        .await;
    assert!(reply["result"].is_object(), "{reply}");
    let reply = client.call("c", "session.create", params).await;
    let session = reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没造出来：{reply}"))
        .to_string();
    client.say("s", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let (_, request) = script.requests().into_iter().next().expect("发了请求");
    (session, request)
}

#[tokio::test]
async fn the_old_no_tools_session_still_carries_the_eight() {
    let home = Home::new();
    home.write("system/config.toml", PROFILE);
    let cwd = home.work.to_string_lossy().into_owned();
    let (session, request) =
        opened(&home, json!({"cwd": cwd, "persona": null, "preset": null})).await;
    assert!(request.tools.is_empty(), "无工具");
    assert_eq!(
        seen(&request),
        [
            "profile",
            "env",
            "cwd",
            "permission",
            "session",
            "memories",
            "local-paths-rule",
            "agent-version",
        ],
        "以前的无工具照旧（权限规则、本机系统跟着工具面，一件工具都没有就不拼）：{}",
        all_of(&request)
    );
    let shown = Shown::of(snapshot(&home, &session).preset.as_ref());
    assert!(shown.facts.model, "没有预设的模型那一块开着");
}

#[tokio::test]
async fn nothing_on_carries_none_of_the_eight() {
    let home = Home::new();
    home.write("system/config.toml", PROFILE);
    let cwd = home.work.to_string_lossy().into_owned();
    let (session, request) = opened(
        &home,
        json!({"cwd": cwd, "persona": null, "preset": "none"}),
    )
    .await;
    assert!(request.tools.is_empty(), "一件工具都没有");
    assert_eq!(
        seen(&request),
        Vec::<&str>::new(),
        "八样都没有：{}",
        all_of(&request)
    );
    let snapshot = snapshot(&home, &session);
    assert_eq!(snapshot.profile, None);
    assert!(!snapshot.no_tools, "是一份真预设，不是无工具");
    let pin = snapshot.preset.as_ref().expect("记了预设");
    let mut all_off = pin.clone();
    all_off.off = miyu_policy::shown::ALL.map(String::from).to_vec();
    assert_eq!(
        Shown::of(Some(pin)),
        Shown::of(Some(&all_off)),
        "环境那几块全关，模型那一块在内"
    );
}

/// 量尺（施工 P-17）：无人格、无工具的会话第一次请求里，system 和消息一块块写进 `MIYU_P17_OUT`，拿 o200k 逐段数。
#[tokio::test]
#[ignore = "量尺：MIYU_P17_OUT=/路径 跑"]
async fn measure_the_no_tools_session() {
    let Ok(out) = std::env::var("MIYU_P17_OUT") else {
        return;
    };
    let home = Home::new();
    home.write("system/config.toml", PROFILE);
    let cwd = home.work.to_string_lossy().into_owned();
    let (_, request) = opened(&home, json!({"cwd": cwd, "persona": null, "preset": null})).await;
    let messages: Vec<String> = request
        .messages
        .iter()
        .map(|message| format!("{message:?}"))
        .collect();
    let dump = json!({"system": request.system, "messages": messages});
    std::fs::File::create(out)
        .and_then(|mut file| file.write_all(dump.to_string().as_bytes()))
        .expect("写得出");
}
