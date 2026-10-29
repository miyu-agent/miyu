//! `miyu setup`（施工 8-11，`docs/blueprint/cli/setup.md`）：在进程里起核心（真目录裁出来的一份，供应商、本机的服务是假
//! 服务器），假终端照剧本回，在真的套接字上走一遍。三个平台一样跑：人那一头是照剧本回的 `Console`（`cli/login.md` 的先例）。
//!
//! 常用的几家、本机的服务、自定义列成一张表（施工 8-11 再补）；环境变量里的 key 只引用不复制；贴的 key 先试、通了存成密钥；
//! 列得出模型就算通、不发请求（施工 8-11 五补）；试不通先问仍然保存不保存，不保存的回到上一步；本机的服务不要 key；已经配好的只写 `models.chat`；核心看不到的变量说清是哪个；屏幕上从头到尾没有 key；
//! `miyu ask` 没模型时先走一遍。配置里一个池都没有的，一起写三个预设的池（施工 8-8 补）。自定义的、取不到模型列表的在
//! `setup_custom.rs`。

use serde_json::json;

use crate::support::Home;
use crate::support::onboarding::{FAKE, Typist, deepseek_at, listing, plan};
use miyu_cli::Setup;
use miyu_http::testkit::{Reply, Server};

const WRONG: &str = "sk-FAKE-WRONG-KEY-0002";

/// 认证失败。
fn unauthorized() -> Reply {
    Reply::error(
        401,
        &[],
        r#"{"error":{"message":"Authentication Fails (no such user)"}}"#,
    )
}

/// 屏幕上「N 毫秒」的数换成 N：每次不一样。
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

/// 屏幕上、配置里没有 key。
fn no_key_on(home: &Home, screen: &str) {
    assert!(!screen.contains("FAKE"), "屏幕上有 key：{screen}");
    assert!(
        !home.system_config().contains("FAKE"),
        "{}",
        home.system_config()
    );
}

#[tokio::test]
async fn a_key_in_the_environment_is_referenced_never_copied() {
    let server = Server::start(vec![listing(&[
        "deepseek-v4-pro",
        "deepseek-flash",
        "a-tiny",
    ])])
    .await;
    let home = Home::onboarding("", &[("DEEPSEEK_API_KEY", FAKE)], deepseek_at(&server));
    let mut typist = Typist::at_terminal(&["3", ""], &[]);
    let here = ["OPENAI_API_KEY", "DEEPSEEK_API_KEY", "NOT_LOOKED_FOR"];
    let asked = home
        .setup(&plan(Setup::default(), &here), &mut typist)
        .await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert_eq!(
        steady(&asked.screen),
        "miyu: 警告：核心未读到 OPENAI_API_KEY（在核心启动后设置）\n提示：核心空闲退出后重新运行 'miyu setup'\n提示：或选择这一家供应商并粘贴密钥\n\
         选择供应商：\n\
         \x20 1  OpenCode Zen\n\
         \x20 2  OpenCode Go\n\
         \x20 3  DeepSeek      已找到密钥\n\
         \x20    Anthropic     不可用：目录中没有地址\n\
         \x20    OpenAI        不可用：目录中没有地址\n\
         \x20 4  自定义\n\
         选择编号：正在测试 DeepSeek…\n\
         已连通：列出 3 个模型\n\
         选主对话的模型：\n\
         \x20 1  deepseek-flash（推荐）\n\
         \x20 2  a-tiny\n\
         \x20 3  deepseek-v4-pro\n\
         选一个编号，直接回车用推荐的：已设置 models.chat = deepseek/deepseek-flash\n"
    );
    let config = home.system_config();
    assert!(
        config.contains("key = { env = \"DEEPSEEK_API_KEY\" }"),
        "只引用：{config}"
    );
    assert!(
        config.contains("chat = \"deepseek/deepseek-flash\""),
        "{config}"
    );
    assert_eq!(home.secrets(), "", "不复制进密钥文件");
    no_key_on(&home, &asked.screen);
    let received = server.received();
    assert_eq!(received.len(), 1, "列到了就不发请求");
    let bearer = format!("Bearer {FAKE}");
    assert_eq!(received[0].header("authorization"), Some(bearer.as_str()));
}

#[tokio::test]
async fn a_pasted_key_is_tried_first_and_kept_only_once_it_works() {
    let server = Server::start(vec![
        unauthorized(),
        listing(&["deepseek-flash", "deepseek-v4-pro"]),
    ])
    .await;
    let home = Home::onboarding("", &[], deepseek_at(&server));
    let mut typist = Typist::at_terminal(&["3", "", "2"], &[WRONG, FAKE]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert_eq!(
        steady(&asked.screen),
        "选择供应商：\n\
         \x20 1  OpenCode Zen\n\
         \x20 2  OpenCode Go\n\
         \x20 3  DeepSeek\n\
         \x20    Anthropic     不可用：目录中没有地址\n\
         \x20    OpenAI        不可用：目录中没有地址\n\
         \x20 4  自定义\n\
         选择编号：粘贴 deepseek 的密钥（不显示）：正在测试 DeepSeek…\n\
         miyu: 连接失败（列模型）：认证失败：HTTP 401: Authentication Fails (no such user)\n\
         仍然保存？[y/N] 粘贴 deepseek 的密钥（不显示）：正在测试 DeepSeek…\n\
         已连通：列出 2 个模型\n\
         已保存 deepseek 的密钥\n\
         选主对话的模型：\n\
         \x20 1  deepseek-flash（推荐）\n\
         \x20 2  deepseek-v4-pro\n\
         选一个编号，直接回车用推荐的：已设置 models.chat = deepseek/deepseek-v4-pro\n"
    );
    assert_eq!(typist.hidden, 2, "试不通、不保存，回到贴 key");
    assert_eq!(server.received().len(), 2, "两次都只列了模型");
    let secrets = home.secrets();
    assert!(
        secrets.contains(&format!("deepseek = \"{FAKE}\"\n")),
        "{secrets}"
    );
    assert!(!secrets.contains(WRONG), "错的那个没存过：{secrets}");
    let config = home.system_config();
    assert!(
        config.contains("key = { secret = \"deepseek\" }"),
        "{config}"
    );
    assert!(
        config.contains("chat = \"deepseek/deepseek-v4-pro\""),
        "{config}"
    );
    no_key_on(&home, &asked.screen);
}

/// 试不通、人说仍然保存（施工 8-11 五补，2026-10-13 项目主人定）：列不出模型的先问模型名，贴的 key 照样存、配置照填的写。
#[tokio::test]
async fn a_failed_try_can_be_saved_anyway() {
    let server = Server::start(vec![unauthorized()]).await;
    let home = Home::onboarding("", &[], deepseek_at(&server));
    let mut typist = Typist::at_terminal(&["3", "Y", " my-model "], &[FAKE]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    let (_, after) = asked
        .screen
        .split_once("粘贴 deepseek 的密钥（不显示）：")
        .expect("问了 key");
    assert_eq!(
        after,
        "正在测试 DeepSeek…\n\
         miyu: 连接失败（列模型）：认证失败：HTTP 401: Authentication Fails (no such user)\n\
         仍然保存？[y/N] 模型名：\
         已保存 deepseek 的密钥\n\
         已设置 models.chat = deepseek/my-model\n"
    );
    assert_eq!(typist.hidden, 1, "保存了就不回去贴 key");
    assert!(
        home.secrets().contains(&format!("deepseek = \"{FAKE}\"\n")),
        "{}",
        home.secrets()
    );
    assert!(
        home.system_config()
            .contains("chat = \"deepseek/my-model\""),
        "{}",
        home.system_config()
    );
    assert_eq!(server.received().len(), 1, "只列了一次模型");
    no_key_on(&home, &asked.screen);
}

/// 仍然保存、模型名却没填：「没选」，退出码 1，key、配置都不写。
#[tokio::test]
async fn saving_anyway_without_a_model_name_is_not_picked() {
    let server = Server::start(vec![unauthorized()]).await;
    let home = Home::onboarding("", &[], deepseek_at(&server));
    let mut typist = Typist::at_terminal(&["3", "yes", "  "], &[FAKE]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 1, "{}", asked.screen);
    assert!(
        asked
            .screen
            .ends_with("仍然保存？[y/N] 模型名：miyu: 未选择\n"),
        "{}",
        asked.screen
    );
    assert_eq!(home.secrets(), "", "没存 key");
    assert_eq!(home.system_config(), "", "配置一个字都没写");
}

/// 贴 key 那一步取消了（施工 8-5 补：`Ctrl+C`，或者空行 `Ctrl+D`），`Console::hidden` 报
/// [`std::io::ErrorKind::Interrupted`]：整个 `miyu setup` 照取消办，退出码 130，配置文件、密钥文件一个字都没写。真的
/// 终端里的 `Ctrl+C` 这一条用的是真的伪终端，在 `crates/miyu/tests/login_tty.rs`。
#[tokio::test]
async fn cancelling_the_key_paste_writes_nothing() {
    let server = Server::start(vec![]).await;
    let home = Home::onboarding("", &[], deepseek_at(&server));
    let mut typist = Typist::at_terminal(&["3"], &[]);
    typist.cancel_key = true;
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 130, "{}", asked.screen);
    assert!(
        asked.screen.ends_with("已取消，未保存\n"),
        "{}",
        asked.screen
    );
    assert_eq!(home.system_config(), "", "配置一个字都没写");
    assert_eq!(home.secrets(), "", "密钥文件一个字都没写");
    assert!(server.received().is_empty(), "没发出去任何请求");
}

#[tokio::test]
async fn a_local_service_needs_no_key() {
    let server = Server::start(vec![listing(&["qwen3-8b"]), listing(&["qwen3-8b"])]).await;
    let profiles =
        json!({"lab": {"name": "Lab", "driver": "openai-chat", "base_url": server.base_url}});
    let home = Home::onboarding("", &[], profiles);
    let mut typist = Typist::at_terminal(&["4", ""], &[]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert!(
        asked.screen.contains(&format!(
            "  3  DeepSeek\n     Anthropic     不可用：目录中没有地址\n     OpenAI        不可用：目录中没有地址\n  4  Lab           本机 {}\n  5  自定义\n",
            server.base_url
        )),
        "{}",
        asked.screen
    );
    assert_eq!(typist.hidden, 0, "不要 key");
    let config = home.system_config();
    assert!(
        config.contains("[providers.lab]\nlocal = true\n"),
        "本机的服务不要 key：写 local = true 算配好了（施工 8-25）：{config}"
    );
    assert!(!config.contains("key"), "{config}");
    assert!(config.contains("chat = \"lab/qwen3-8b\""), "{config}");
    assert_eq!(server.received()[1].header("authorization"), None);
}

#[tokio::test]
async fn a_provider_already_set_up_only_gets_models_chat() {
    let server = Server::start(vec![listing(&["deepseek-flash"])]).await;
    let config = "[providers.ds]\ncatalog = \"deepseek\"\nkey = { env = \"DEEPSEEK_API_KEY\" }\nprice_multiplier = 0.5\n";
    let home = Home::onboarding(config, &[("DEEPSEEK_API_KEY", FAKE)], deepseek_at(&server));
    let mut typist = Typist::at_terminal(&["3", ""], &[]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert!(
        asked.screen.contains("  3  DeepSeek      已配置\n"),
        "{}",
        asked.screen
    );
    let written = home.system_config();
    assert!(written.starts_with(config), "原来的一个字没动：{written}");
    assert!(
        written.contains("chat = \"ds/deepseek-flash\""),
        "{written}"
    );
}

#[tokio::test]
async fn nothing_picked_or_no_key_ends_with_1() {
    let home = Home::onboarding("", &[("DEEPSEEK_API_KEY", FAKE)], json!({}));
    let mut typist = Typist::at_terminal(&[""], &[]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 1);
    assert!(
        asked.screen.ends_with("选择编号：miyu: 未选择\n"),
        "{}",
        asked.screen
    );

    let mut typist = Typist::at_terminal(&["9", "x"], &[]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert!(
        asked.screen.contains(
            "选择编号：miyu: 无效的编号：9\n选择编号：miyu: 无效的编号：x\n选择编号：miyu: 未选择\n"
        ),
        "{}",
        asked.screen
    );

    // opencode Zen：环境里没有它的 key，要贴（DeepSeek 找到了 key，选它就真的发出去了）。
    let mut typist = Typist::at_terminal(&["1"], &["  "]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 1);
    assert!(
        asked.screen.ends_with("（不显示）：miyu: 未收到密钥\n"),
        "{}",
        asked.screen
    );
    assert_eq!(home.system_config(), "", "什么都没写");
}

#[tokio::test]
async fn ask_without_a_model_goes_through_setup_first_at_a_terminal() {
    let server = Server::start(vec![listing(&["deepseek-flash"])]).await;
    let home = Home::onboarding("", &[("DEEPSEEK_API_KEY", FAKE)], deepseek_at(&server));
    let mut piped = Typist::piping("");
    let (ready, asked) = home.ready(&plan(Setup::default(), &[]), &mut piped).await;
    assert_eq!(ready, Err(5));
    assert_eq!(
        asked.err,
        "miyu: 未配置模型\n提示：运行 'miyu setup' 添加模型\n"
    );
    assert_eq!(home.sessions().len(), 0, "不造会话");

    let mut typist = Typist::at_terminal(&["3", ""], &[]);
    let (ready, asked) = home.ready(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(ready, Ok(()), "{}", asked.screen);
    assert!(
        asked
            .screen
            .starts_with("未配置模型，开始设置\n选择供应商：\n"),
        "{}",
        asked.screen
    );
    assert!(
        home.system_config()
            .contains("chat = \"deepseek/deepseek-flash\"")
    );

    let mut untouched = Typist::at_terminal(&[], &[]);
    let (ready, asked) = home
        .ready(&plan(Setup::default(), &[]), &mut untouched)
        .await;
    assert_eq!(ready, Ok(()), "有了就不问：{}", asked.screen);
    assert_eq!(asked.screen, "");
    assert_eq!(untouched.lines, 0);
}

/// 不再预写池（施工 Q-1，设计 43 第三节第 1 条）：三个固定池 `lite`、`normal`、`flagship` 本来就在（空的），`miyu setup`
/// 只写 `models.chat`，一个池都不写。屏幕上照旧只说 `models.chat`。
#[tokio::test]
async fn setup_writes_no_pools() {
    let server = Server::start(vec![listing(&["deepseek-flash"])]).await;
    let home = Home::onboarding("", &[("DEEPSEEK_API_KEY", FAKE)], deepseek_at(&server));
    let mut typist = Typist::at_terminal(&["3", ""], &[]);
    let asked = home.setup(&plan(Setup::default(), &[]), &mut typist).await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert!(
        asked
            .screen
            .ends_with("已设置 models.chat = deepseek/deepseek-flash\n"),
        "{}",
        asked.screen
    );
    let written = home.system_config();
    assert!(!written.contains("[pools."), "一个池都不写：{written}");
}

/// 系统配置里已经有 `models.chat` 的（施工 T-11，网页的第一次引导先碰上的）：换上的就是新选的。
#[tokio::test]
async fn a_chat_model_already_written_is_replaced() {
    let server = Server::start(vec![listing(&[
        "deepseek-v4-pro",
        "deepseek-flash",
        "a-tiny",
    ])])
    .await;
    let home = Home::onboarding(
        "[models]\nchat = \"magpie/old-model\"\n",
        &[("DEEPSEEK_API_KEY", FAKE)],
        deepseek_at(&server),
    );
    let mut typist = Typist::at_terminal(&["3", ""], &[]);
    let asked = home
        .setup(&plan(Setup::default(), &["DEEPSEEK_API_KEY"]), &mut typist)
        .await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    let written = home.system_config();
    assert!(
        written.contains("chat = \"deepseek/deepseek-flash\""),
        "换成新选的：{written}"
    );
    assert!(!written.contains("magpie/old-model"), "{written}");
}

/// 列出来的模型全列（施工 ST-2，项目主人 10-29 在虚拟机里接 OpenCode Go：只列 20 个、说「还有 17 个」）：第 25 个也有编号，选得上。
#[tokio::test]
async fn every_listed_model_gets_a_number() {
    let names: Vec<String> = (1..=25).map(|at| format!("model-{at:02}")).collect();
    let listed: Vec<&str> = names.iter().map(String::as_str).collect();
    let server = Server::start(vec![listing(&listed)]).await;
    let home = Home::onboarding("", &[("DEEPSEEK_API_KEY", FAKE)], deepseek_at(&server));
    let mut typist = Typist::at_terminal(&["3", "25"], &[]);
    let asked = home
        .setup(&plan(Setup::default(), &["DEEPSEEK_API_KEY"]), &mut typist)
        .await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    let last = asked
        .screen
        .lines()
        .find_map(|line| line.strip_prefix("  25  "))
        .unwrap_or_else(|| panic!("第 25 个有编号：{}", asked.screen));
    assert!(!asked.screen.contains("还有"), "{}", asked.screen);
    let wanted = format!("chat = \"deepseek/{}\"", last.trim());
    assert!(
        home.system_config().contains(&wanted),
        "{wanted}：{}",
        home.system_config()
    );
}
