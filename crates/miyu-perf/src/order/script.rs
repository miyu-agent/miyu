//! 排函数的剧本（施工 LW-1）：沙盒怎么配、连上以后照先后做哪几样。`order.rs` 管起停核心、录日志、写清单。
//!
//! 沙盒的系统配置配三家供应商：原来的 `perf`（明文），`perftls`（HTTPS、HTTP/1.1），`perfh2`（HTTPS、HTTP/2），各一个窗口
//! [`WINDOW`] 的模型 `perf-model`。会话先用 `models.chat`（`perf/perf-model`），再照 `session.configure` 换到另外两家各说
//! 一句：真用的人换模型也是这样换。HTTPS 的两句要假模型那个口真的收到了才算（[`Arrival::via`](crate::fake::Arrival::via)），
//! 不悄悄退回明文。

use std::path::Path;
use std::time::Duration;

use serde_json::json;

use crate::fake::{Fake, Secure, Transport, pem};
use crate::measure::sessions::HEAD_ASKS;
use crate::measure::tools::calls::{KINDS, Workspace};
use crate::measure::{FORGETFUL, REMEMBERING, open, say, subscribe};
use crate::rpc::Rpc;
use crate::sandbox::{Sandbox, WINDOW};

/// 三个模型：照先后说，各走哪个口。
const MODELS: [(&str, Transport); 3] = [
    ("perf/perf-model", Transport::Plain),
    ("perftls/perf-model", Transport::Http1),
    ("perfh2/perf-model", Transport::H2),
];

/// 一段录完以后歇多久再停：核心最后一个请求过去 2 秒还内存、记账（`heap.md` 第 2 条），这些也要录到。
pub const QUIET: Duration = Duration::from_secs(4);

/// 抽记忆那一段录完以后歇多久再停：最后一次合并记下以后，核心还要关掉整理用的会话、清掉旧的整理会话、把它退掉。
const SETTLED: Duration = Duration::from_secs(12);

/// 系统的证书库常在的几处（`rustls-native-certs` 照 `openssl-probe` 找的那几处里常见的）：有的接在信任的那份前面，核心读
/// 证书时和真用的人一样要读一整批。
const SYSTEM_BUNDLES: [&str; 3] = [
    "/etc/ssl/certs/ca-certificates.crt",
    "/etc/pki/tls/certs/ca-bundle.crt",
    "/etc/ssl/cert.pem",
];

/// 往沙盒的系统配置里加上两家 HTTPS 的供应商，地址是 `secure` 的两个口。核心起来以前调。
///
/// # Errors
///
/// 配置读不了、写不进。
pub fn configure(sandbox: &Sandbox, secure: &Secure) -> Result<(), String> {
    let path = sandbox.root.config_file();
    let mut config = std::fs::read_to_string(&path).map_err(|e| format!("系统配置读不了：{e}"))?;
    for (name, url) in [("perftls", &secure.http1), ("perfh2", &secure.h2)] {
        config.push_str(&format!(
            "\n[providers.{name}]\ndriver = \"openai-chat\"\nbase_url = \"{url}\"\nkey = {{ env = \"MIYU_PERF_KEY\" }}\n\n[providers.{name}.models.perf-model]\nwindow = {WINDOW}\n"
        ));
    }
    std::fs::write(&path, config).map_err(|e| format!("系统配置写不进：{e}"))
}

/// 在 `path` 写核心的 `SSL_CERT_FILE`：系统的证书库有就接上，再加假模型的测试证书。
///
/// # Errors
///
/// 系统的证书库读不了（在却读不了）、写不进。
pub fn trust(path: &Path) -> Result<(), String> {
    let mut text = String::new();
    if let Some(bundle) = SYSTEM_BUNDLES.iter().map(Path::new).find(|p| p.is_file()) {
        text = std::fs::read_to_string(bundle)
            .map_err(|e| format!("读不了系统的证书库 {}：{e}", bundle.display()))?;
        if !text.ends_with('\n') {
            text.push('\n');
        }
    }
    text.push_str(&pem());
    std::fs::write(path, text).map_err(|e| format!("写不进 {}：{e}", path.display()))
}

/// 终端界面起来时除了 [`HEAD_ASKS`] 还发的几样（照 2026-11-21 钉在 `heads.toml` 的终端界面真发的抄，参数也照它）：订阅配置、
/// 会话列表、软件包、通知、定时、总线、部件几条流，问会话上下文、命令目录。量尺只问六样的时候，这些路上的三百多个函数
/// 散在清单以外，真用时每碰一处带进一片邻居页（实测核心空闲的代码页 9.1 MB，录上以后 4.6 MB）。头改了问法，这里跟着改。
fn head_extras() -> Vec<(&'static str, serde_json::Value)> {
    let stream = |name: &str| ("subscribe", json!({"stream": name}));
    vec![
        ("config.get", json!({"keys": ["ui.startup"]})),
        stream("config"),
        (
            "config.get",
            json!({"keys": ["ui.language", "usage.currency", "persona.default", "preset.default"]}),
        ),
        stream("sessions"),
        stream("packages"),
        stream("notices"),
        stream("schedules"),
        stream("bus"),
        ("session.viewing", json!({"session": null})),
        ("session.context", json!({"persona": null})),
        ("human.get", json!({"language": "zh"})),
        ("command.catalog", json!({})),
        (
            "subscribe",
            json!({"stream": "widgets", "showing": ["home", "status"]}),
        ),
    ]
}

/// 像头那样连上、握手、问一遍 [`HEAD_ASKS`] 和 [`head_extras`]：交回连接。
///
/// # Errors
///
/// 连不上、哪一样回了错。
pub async fn head(sandbox: &Sandbox) -> Result<Rpc, String> {
    let mut rpc = Rpc::connect(&sandbox.root).await?;
    for method in HEAD_ASKS {
        rpc.call(method, json!({}))
            .await
            .map_err(|e| format!("头起来时问 {method}：{e}"))?;
    }
    for (method, params) in head_extras() {
        rpc.call(method, params.clone())
            .await
            .map_err(|e| format!("头起来时问 {method} {params}：{e}"))?;
    }
    Ok(rpc)
}

/// 头打开一个会话以后发的几样（同样照终端界面真发的抄）：订阅这个会话的部件、说正在看它、问它的用量。
///
/// # Errors
///
/// 哪一样回了错。
async fn watching(rpc: &mut Rpc, session: &str) -> Result<(), String> {
    let asks = [
        (
            "subscribe",
            json!({"stream": "widgets", "session": session, "showing": ["sidebar", "status"]}),
        ),
        ("session.viewing", json!({"session": session})),
        ("usage.query", json!({"session": session, "tree": true})),
    ];
    for (method, params) in asks {
        rpc.call(method, params.clone())
            .await
            .map_err(|e| format!("头看着会话时问 {method} {params}：{e}"))?;
    }
    Ok(())
}

/// 开一个关着记忆的会话，在 [`MODELS`] 上各说一句（记号是 `<tag>-<第几个>`）：交回会话的编号。
///
/// # Errors
///
/// 造不了会话、换不了模型、哪一句没说成、HTTPS 的那句没从那个口来。
pub async fn three_models(
    rpc: &mut Rpc,
    fake: &mut Fake,
    sandbox: &Sandbox,
    tag: &str,
) -> Result<String, String> {
    let session = open(rpc, &sandbox.work, FORGETFUL).await?;
    for (n, (model, via)) in MODELS.into_iter().enumerate() {
        if n > 0 {
            rpc.call(
                "session.configure",
                json!({"session": session, "model": model}),
            )
            .await
            .map_err(|e| format!("换到 {model}：{e}"))?;
        }
        say_via(rpc, fake, &session, &format!("{tag}-{n}"), via).await?;
    }
    Ok(session)
}

/// 说一句带 `marker` 的，要假模型从 `via` 那个口收到它这一轮的请求，等这一轮结束。从别的口来的带记号的（起标题这些辅助
/// 请求可能走别的模型）跳过接着等；一直等不到的报出来，带着从哪几个口收到过。
async fn say_via(
    rpc: &mut Rpc,
    fake: &mut Fake,
    session: &str,
    marker: &str,
    via: Transport,
) -> Result<(), String> {
    rpc.call(
        "session.send",
        json!({"session": session, "text": format!("{marker}: keep going.")}),
    )
    .await?;
    let mut elsewhere = Vec::new();
    loop {
        let arrival = fake.request_with(marker).await.map_err(|e| {
            format!("等「{marker}」从 {via:?} 的口来（别的口收到过 {elsewhere:?}）：{e}")
        })?;
        if arrival.via == via {
            break;
        }
        elsewhere.push(arrival.via);
    }
    rpc.turn_ended(session).await
}

/// 第一段（在模拟器里）：头起来时问的那些、三个模型各一句、头看着会话时问的几样、开着记忆的会话一句、工具一步每种调一次（工作区 `workspace`，守卫、
/// `tool.after`、扩展驱动那几种不做）、歇 [`QUIET`]。交回关着记忆的那个会话，第二段打开它。
///
/// # Errors
///
/// 哪一步没成，报是在做哪一步。
pub async fn first(
    sandbox: &Sandbox,
    fake: &mut Fake,
    workspace: &Workspace,
) -> Result<String, String> {
    let mut rpc = head(sandbox).await?;
    let session = three_models(&mut rpc, fake, sandbox, "perf-order-first").await?;
    watching(&mut rpc, &session).await?;
    let remembering = open(&mut rpc, &sandbox.work, REMEMBERING).await?;
    say(&mut rpc, fake, &remembering, "perf-order-remembering")
        .await
        .map_err(|e| format!("开着记忆的会话：{e}"))?;
    for (index, kind) in KINDS.iter().enumerate() {
        crate::measure::tools::steps(&mut rpc, fake, workspace, 0, index, kind)
            .await
            .map_err(|e| format!("工具一步「{}」：{e}", kind.name))?;
    }
    fake.script(None);
    tokio::time::sleep(QUIET).await;
    Ok(session)
}

/// 第二段（在模拟器里，同一个数据根）：头起来时问的那些，订阅第一段那个关着记忆的会话（核心从日志载入它），说一句，头看着
/// 会话时问的几样，歇 [`QUIET`]。
///
/// # Errors
///
/// 哪一步没成。
pub async fn second(sandbox: &Sandbox, fake: &mut Fake, session: &str) -> Result<(), String> {
    let mut rpc = head(sandbox).await?;
    subscribe(&mut rpc, session)
        .await
        .map_err(|e| format!("打开第一段的会话：{e}"))?;
    say(&mut rpc, fake, session, "perf-order-second").await?;
    watching(&mut rpc, session).await?;
    tokio::time::sleep(QUIET).await;
    Ok(())
}

/// 抽记忆的那一段（在模拟器里，同一个数据根，施工 LW-14，`order/remember.rs`；沙箱配置要先加上
/// [`super::remember::SETTINGS`]）：头起来时问的那些，开一个开着记忆的新会话，每说够一次抽取的轮数就等它抽完一次，一共抽
/// 两次（轮数和次数的来由见 `order/remember.rs` 开头），歇 [`SETTLED`]。
///
/// 这一段单独录、日志排在核心清单的最后：清单照头一回执行的先后排，抽记忆要用的函数里有一部分平常对话、载入会话也用。
/// 抽记忆录在前面几段中间的话，这一部分函数抢先在这里头一回执行，和只有抽记忆才用的函数排在一起，平常每执行到一个就
/// 带进一片用不着的邻居页（2026-11-22 实测：录在第一段里，核心的代码页在空闲、说过一句、九次工具调用三个状态都多
/// 0.26 到 0.45 MB）。录在最后，前面几段的清单和没有这一段时一样，新执行到的只接在后面。
///
/// # Errors
///
/// 哪一步没成，报是在做哪一步。
pub async fn remembered(sandbox: &Sandbox, fake: &mut Fake) -> Result<(), String> {
    use super::remember::{ROUNDS, TURNS, extracted};
    let mut rpc = head(sandbox).await?;
    let session = open(&mut rpc, &sandbox.work, REMEMBERING).await?;
    fake.organize(true);
    let talked = async {
        for round in 1..=ROUNDS {
            for turn in 1..=TURNS {
                let marker = format!("perf-order-remembered-{round}-{turn}");
                say(&mut rpc, fake, &session, &marker).await.map_err(|e| {
                    format!("开着记忆的会话第 {round} 次抽取以前的第 {turn} 句：{e}")
                })?;
            }
            extracted(sandbox, fake, &session, round).await?;
        }
        Ok::<_, String>(())
    }
    .await;
    fake.organize(false);
    talked?;
    tokio::time::sleep(SETTLED).await;
    Ok(())
}
