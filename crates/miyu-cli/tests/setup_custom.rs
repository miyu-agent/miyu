//! `miyu setup` 的自定义一家、取不到模型列表（施工 8-11 再补，`docs/blueprint/cli/setup.md`「怎么走」第 4、7 条）：依次问
//! base URL、接口协议、key（可以空）；配置里的编号照主机名起、撞了往后加，试通了问一句，直接回车用它、也能自己起（施工
//! 8-11 六补）；空 key 的不存、不写密钥。列不出模型的印
//! 「无法获取模型列表」和原因、问模型名，拿它发一句再试（施工 8-11 五补：列得出的不发请求），通了不再列、不再问。三个平台一样跑。

use serde_json::json;

use crate::support::Home;
use crate::support::onboarding::{FAKE, Typist, answer, listing, plan, remote};
use miyu_cli::Setup;
use miyu_http::testkit::{Reply, Server};

/// 表里「自定义」那一行的编号：DeepSeek、opencode Zen 之后（OpenAI、Anthropic 在裁出来的目录里没有地址，不编号）。
const CUSTOM: &str = "4";

/// 列模型那一下：没有这个接口。
fn no_listing() -> Reply {
    Reply::error(404, &[], r#"{"error":{"message":"not found"}}"#)
}

/// 屏幕上「N 毫秒」的数换成 N。
fn steady(screen: &str) -> String {
    let mut out = String::new();
    for line in screen.split_inclusive('\n') {
        match line.split_once("，") {
            Some((head, tail)) if head.contains("已连通") => {
                let rest = tail.split_once(" 毫秒").map_or(tail, |(_, rest)| rest);
                out.push_str(&format!("{head}，N 毫秒{rest}"));
            }
            _ => out.push_str(line),
        }
    }
    out
}

/// 敲的那几行要是 `&'static str`：地址是起了服务器才有的。
fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

#[tokio::test]
async fn a_custom_provider_asks_address_protocol_and_key_and_is_named_after_its_host() {
    let server = Server::start(vec![listing(&["m-small", "m-large"])]).await;
    let home = Home::onboarding("", &[], json!({}));
    let base_url = leak(format!("{}/", remote(&server)));
    let mut typist = Typist::at_terminal(&[CUSTOM, "example.com", base_url, "1", "", "2"], &[FAKE]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    let shown = steady(&asked.screen);
    let (_, after) = shown.split_once("  4  自定义\n").expect("有自定义那一行");
    assert_eq!(
        after,
        "选择编号：Base URL：miyu: 地址必须以 http:// 或 https:// 开头\n\
         Base URL：接口协议：\n\
         \x20 1  OpenAI 兼容\n\
         \x20 2  Anthropic\n\
         \x20 3  OpenAI Responses\n\
         选择编号：密钥（不显示，可留空）：正在测试 127.1…\n\
         已连通：列出 2 个模型\n\
         ID（回车使用 local）：已保存 local 的密钥\n\
         选主对话的模型：\n\
         \x20 1  m-large（推荐）\n\
         \x20 2  m-small\n\
         选一个编号，直接回车用推荐的：已设置 models.chat = local/m-small\n"
    );
    let config = home.system_config();
    let wanted = format!(
        "[providers.local]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nkey = {{ secret = \"local\" }}\n",
        remote(&server)
    );
    assert!(config.contains(&wanted), "{config}");
    assert!(config.contains("chat = \"local/m-small\""), "{config}");
    assert!(
        home.secrets().contains(&format!("local = \"{FAKE}\"\n")),
        "{}",
        home.secrets()
    );
    let bearer = format!("Bearer {FAKE}");
    let received = server.received();
    assert_eq!(received.len(), 1, "列到了就不发请求");
    assert_eq!(received[0].header("authorization"), Some(bearer.as_str()));
}

#[tokio::test]
async fn a_taken_id_gets_a_number_and_an_empty_key_stores_nothing() {
    let server = Server::start(vec![listing(&["m-small"])]).await;
    let config = "[providers.local]\ndriver = \"anthropic\"\nbase_url = \"http://127.0.0.1:1\"\n";
    let home = Home::onboarding(config, &[], json!({}));
    let base_url = leak(remote(&server));
    let mut typist = Typist::at_terminal(&[CUSTOM, base_url, "1", "", ""], &["  "]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert!(
        asked.screen.contains("ID（回车使用 local-2）："),
        "{}",
        asked.screen
    );
    let written = home.system_config();
    assert!(written.starts_with(config), "原来的一个字没动：{written}");
    let wanted =
        format!("[providers.local-2]\ndriver = \"openai-chat\"\nbase_url = \"{base_url}\"\n");
    assert!(written.contains(&wanted), "{written}");
    assert!(written.contains("chat = \"local-2/m-small\""), "{written}");
    assert_eq!(home.secrets(), "", "空 key 不存");
    assert_eq!(server.received()[0].header("authorization"), None);
    assert!(!asked.screen.contains("已保存"), "{}", asked.screen);
}

#[tokio::test]
async fn without_a_model_list_the_model_name_is_asked_and_tried() {
    let server = Server::start(vec![no_listing(), no_listing(), answer()]).await;
    let home = Home::onboarding("", &[], json!({}));
    let base_url = leak(remote(&server));
    let mut typist = Typist::at_terminal(&[CUSTOM, base_url, "1", " my-model ", ""], &[""]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    let shown = steady(&asked.screen);
    let (_, after) = shown
        .split_once("密钥（不显示，可留空）：")
        .expect("问了 key");
    assert_eq!(
        after,
        "正在测试 127.1…\n\
         miyu: 警告：无法获取模型列表，目录里也没有这一家的模型：other: HTTP 404: not found\n\
         模型名：正在测试 127.1…\n\
         已连通：测试 my-model，N 毫秒收到第一个字\n\
         ID（回车使用 local）：已设置 models.chat = local/my-model\n"
    );
    let sent: serde_json::Value =
        serde_json::from_slice(&server.received()[2].body).expect("发的是 JSON");
    assert_eq!(sent["model"], json!("my-model"));
    assert!(
        home.system_config().contains("chat = \"local/my-model\""),
        "{}",
        home.system_config()
    );
}

#[tokio::test]
async fn an_empty_model_name_or_url_is_not_picked_and_writes_nothing() {
    let server = Server::start(vec![no_listing()]).await;
    let home = Home::onboarding("", &[], json!({}));
    let base_url = leak(remote(&server));
    let mut typist = Typist::at_terminal(&[CUSTOM, base_url, "1", ""], &[""]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 1, "{}", asked.screen);
    assert!(
        asked
            .screen
            .ends_with("没有这一家的模型：other: HTTP 404: not found\n模型名：miyu: 未选择\n"),
        "{}",
        asked.screen
    );

    let mut typist = Typist::at_terminal(&[CUSTOM, ""], &[]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 1, "{}", asked.screen);
    assert!(
        asked.screen.ends_with("Base URL：miyu: 未选择\n"),
        "{}",
        asked.screen
    );
    assert_eq!(home.system_config(), "", "什么都没写");
    assert_eq!(home.secrets(), "");
}

/// 自己起编号（施工 8-11 六补，2026-10-13 项目主人照推荐定）：不合写法的、配置里已经有了的说一句再问，合的就用它，key 存成这个名字；
/// 读到头的是「没选」，什么都不写。
#[tokio::test]
async fn the_id_can_be_typed_and_a_bad_or_taken_one_is_asked_again() {
    let server = Server::start(vec![listing(&["m-small"])]).await;
    let config = "[providers.lan-box]\ndriver = \"anthropic\"\nbase_url = \"http://127.0.0.1:1\"\n";
    let home = Home::onboarding(config, &[], json!({}));
    let base_url = leak(remote(&server));
    let mut typist = Typist::at_terminal(
        &[
            CUSTOM,
            base_url,
            "1",
            "Lan Box",
            "lan-box",
            " office-gpu ",
            "",
        ],
        &[FAKE],
    );
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    let (_, after) = asked
        .screen
        .split_once("已连通：列出 1 个模型\n")
        .expect("试通了");
    assert_eq!(
        after,
        "ID（回车使用 local）：miyu: ID 无效：Lan Box\n提示：以小写字母开头，只含小写字母、数字、- 和 _，最多 32 个字符\n\
         ID（回车使用 local）：miyu: ID 'lan-box' 已存在\n\
         ID（回车使用 local）：已保存 office-gpu 的密钥\n\
         选主对话的模型：\n\
         \x20 1  m-small（推荐）\n\
         选一个编号，直接回车用推荐的：已设置 models.chat = office-gpu/m-small\n"
    );
    let written = home.system_config();
    let wanted = format!(
        "[providers.office-gpu]\ndriver = \"openai-chat\"\nbase_url = \"{base_url}\"\nkey = {{ secret = \"office-gpu\" }}\n"
    );
    assert!(written.contains(&wanted), "{written}");
    assert!(
        home.secrets()
            .contains(&format!("office-gpu = \"{FAKE}\"\n")),
        "{}",
        home.secrets()
    );

    let server = Server::start(vec![listing(&["m-small"])]).await;
    let home = Home::onboarding("", &[], json!({}));
    let base_url = leak(remote(&server));
    let mut typist = Typist::at_terminal(&[CUSTOM, base_url, "1"], &[FAKE]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 1, "{}", asked.screen);
    assert!(
        asked
            .screen
            .ends_with("ID（回车使用 local）：miyu: 未选择\n"),
        "{}",
        asked.screen
    );
    assert_eq!(home.system_config(), "", "什么都没写");
    assert_eq!(home.secrets(), "");
}
