//! 重场景的后五段（施工 LW-30）：记忆的整理、MCP、定时任务和循环、抓网页和搜索、软件包和命令行。写法同 `heavy/segments.rs`。

use serde_json::json;

use super::mcp::ARG;
use super::segments::ready;
use super::steps::{Ctx, ask, cli, open, servers_running, talk, until, utc_after};
use super::web::follow;
use crate::order::script::{QUIET, head};

/// 人格：出厂的软件工程师人格，记忆跟着它。
const PERSONA: &str = "software-engineer";

/// 第五段：记忆的整理。人记三条（用户、反馈、认识的人）、列、搜、搜以前的对话，改一条、作废一条再恢复，人卡、改人卡，
/// 整理一次（假模型照真的模型那样回）；开着记忆的会话里模型记一条、搜、查历史、查用量，再说三句，会话里 `/dream`；命令行
/// `miyu memory list`、`miyu memory search`。
///
/// # Errors
///
/// 哪一步没成。
pub async fn memory(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let mut rpc = head(ctx.sandbox).await?;
    ready(&mut rpc).await?;
    ask(
        &mut rpc,
        "subscribe",
        json!({"stream": "memory", "persona": PERSONA}),
    )
    .await?;
    let mut ids = Vec::new();
    for (class, text) in [
        ("user", "The owner measures memory use."),
        ("feedback", "Prefer measured numbers over guesses."),
        ("person", "Xiao Lin asked about ordering."),
    ] {
        let mut params = json!({"persona": PERSONA, "class": class, "text": text});
        if class == "person" {
            params["who"] = json!("qq:20001");
        }
        ids.push(ask(&mut rpc, "memory.remember", params).await?["id"].clone());
    }
    ask(&mut rpc, "memory.list", json!({"persona": PERSONA})).await?;
    ask(
        &mut rpc,
        "memory.search",
        json!({"persona": PERSONA, "query": "memory"}),
    )
    .await?;
    ask(
        &mut rpc,
        "memory.turns",
        json!({"persona": PERSONA, "query": "排函数"}),
    )
    .await?;
    ask(&mut rpc, "memory.update", json!({"persona": PERSONA, "id": ids[0], "text": "The owner measures memory use carefully."})).await?;
    ask(
        &mut rpc,
        "memory.forget",
        json!({"persona": PERSONA, "id": ids[1], "why": "measuring"}),
    )
    .await?;
    ask(
        &mut rpc,
        "memory.restore",
        json!({"persona": PERSONA, "id": ids[1]}),
    )
    .await?;
    ask(&mut rpc, "memory.cards", json!({"persona": PERSONA})).await?;
    ask(
        &mut rpc,
        "memory.card_update",
        json!({"persona": PERSONA, "who": "qq:20001", "text": "小林，常问排函数的事。"}),
    )
    .await?;
    ctx.fake.organize(true);
    let organized = async {
        ask(&mut rpc, "memory.dream", json!({"persona": PERSONA})).await?;
        let session = open(
            ctx,
            &mut rpc,
            json!({"persona": PERSONA, "memory": "persona"}),
        )
        .await?;
        let calls = [
            (
                "remember",
                json!({"class": "user", "text": "The owner likes green tea."}),
            ),
            ("memory_search", json!({"query": "tea"})),
            ("history", json!({"query": "tea"})),
            ("session_usage", json!({})),
        ];
        talk(ctx, &mut rpc, &session, "memory", &calls).await?;
        for _ in 0..3 {
            talk(ctx, &mut rpc, &session, "memory-chat", &[]).await?;
        }
        ask(
            &mut rpc,
            "command.run",
            json!({"session": session, "text": "/dream"}),
        )
        .await?;
        Ok::<_, String>(())
    }
    .await;
    ctx.fake.organize(false);
    organized?;
    cli(ctx, &["memory", "list", "--persona", PERSONA])?;
    cli(ctx, &["memory", "search", "tea", "--persona", PERSONA])?;
    tokio::time::sleep(QUIET).await;
    Ok(())
}

/// 第六段：MCP。会话里模型调两个服务的工具和资源；命令目录、服务的提示词命令；要登录的那个服务走一遍 OAuth（量尺替人
/// 打开授权页、跟着跳回核心的回调），等它连上、调它一次，再登出；重启一个，加一个、看列表、删掉。
///
/// # Errors
///
/// 哪一步没成。
pub async fn mcp(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let mut rpc = head(ctx.sandbox).await?;
    ready(&mut rpc).await?;
    let session = open(ctx, &mut rpc, json!({})).await?;
    let calls = [
        ("mcp__perfstdio__echo", json!({"text": "hi"})),
        ("mcp__perfstdio__add", json!({"a": 2, "b": 3})),
        ("mcp__perfstdio__list_resources", json!({})),
        (
            "mcp__perfstdio__read_resource",
            json!({"uri": "file:///notes.txt"}),
        ),
        ("mcp__perfhttp__echo", json!({"text": "hi"})),
        ("mcp__perfhttp__note", json!({"text": "n"})),
        (
            "mcp__perfhttp__read_resource",
            json!({"uri": "file:///readme.md"}),
        ),
    ];
    talk(ctx, &mut rpc, &session, "mcp", &calls).await?;
    ask(&mut rpc, "command.catalog", json!({"session": session})).await?;
    ask(
        &mut rpc,
        "command.run",
        json!({"session": session, "text": "/perfstdio:hello"}),
    )
    .await?;
    let login = ask(&mut rpc, "mcp.login", json!({"service": "perfauth"})).await?;
    let url = login["url"]
        .as_str()
        .ok_or_else(|| format!("mcp.login 没交授权页：{login}"))?;
    follow(url, 3)
        .await
        .map_err(|e| format!("打开授权页：{e}"))?;
    until(
        &mut rpc,
        "登录以后服务连上",
        "mcp.list",
        json!({}),
        |l| servers_running(l, &["perfauth"]),
    )
    .await?;
    talk(
        ctx,
        &mut rpc,
        &session,
        "mcp-auth",
        &[("mcp__perfauth__echo", json!({"text": "after login"}))],
    )
    .await?;
    ask(&mut rpc, "mcp.logout", json!({"service": "perfauth"})).await?;
    ask(&mut rpc, "mcp.restart", json!({"name": "perfstdio"})).await?;
    let server = ctx.placed.server.to_string_lossy().into_owned();
    ask(
        &mut rpc,
        "mcp.set",
        json!({"name": "perfextra", "command": server, "args": [ARG]}),
    )
    .await?;
    until(&mut rpc, "加的服务在跑", "mcp.list", json!({}), |l| {
        servers_running(l, &["perfextra"])
    })
    .await?;
    ask(&mut rpc, "mcp.remove", json!({"name": "perfextra"})).await?;
    tokio::time::sleep(QUIET).await;
    Ok(())
}

/// 第七段：定时任务和循环。一个 4 秒以后新开会话跑一次的，一个每小时在已有会话上跑的（暂停、恢复、改、取消）；等前一个
/// 跑完一轮（`notice` 的 `schedule.ran`）；会话里模型建一个、列一遍，`/loop` 开一个再关掉；命令行 `miyu schedule list`。
///
/// # Errors
///
/// 哪一步没成。
pub async fn schedules(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let mut rpc = head(ctx.sandbox).await?;
    ready(&mut rpc).await?;
    let at = utc_after(4)?;
    ask(&mut rpc, "schedule.add", json!({"when": {"at": at}, "prompt": "perf-heavy-scheduled: look once.", "target": {"new": {}}})).await?;
    let session = open(ctx, &mut rpc, json!({})).await?;
    let hourly = ask(
        &mut rpc,
        "schedule.add",
        json!({"when": {"every": 3600}, "prompt": "perf-heavy-hourly: look.",
        "target": {"session": session}}),
    )
    .await?;
    ask(&mut rpc, "schedule.list", json!({})).await?;
    let id = hourly["schedule"]["id"].clone();
    ask(&mut rpc, "schedule.pause", json!({"id": id})).await?;
    ask(&mut rpc, "schedule.resume", json!({"id": id})).await?;
    ask(
        &mut rpc,
        "schedule.update",
        json!({"id": id, "prompt": "perf-heavy-hourly: look again."}),
    )
    .await?;
    ask(&mut rpc, "schedule.cancel", json!({"id": id})).await?;
    rpc.push("定时任务跑完一轮", |p| {
        p["method"] == "notice" && p["params"]["kind"] == "schedule.ran"
    })
    .await?;
    let calls = [
        (
            "cron_create",
            json!({"cron": "*/30 * * * *", "prompt": "perf-heavy-half-hour", "recurring": true}),
        ),
        ("cron_list", json!({})),
    ];
    talk(ctx, &mut rpc, &session, "schedule", &calls).await?;
    ask(
        &mut rpc,
        "command.run",
        json!({"session": session, "text": "/loop 10m perf-heavy-loop look"}),
    )
    .await?;
    ask(
        &mut rpc,
        "command.run",
        json!({"session": session, "text": "/loop off"}),
    )
    .await?;
    cli(ctx, &["schedule", "list"])?;
    tokio::time::sleep(QUIET).await;
    Ok(())
}

/// 第八段：抓网页和搜索。会话里模型经假代理抓两页（一页翻到第 2 页），搜一次（SearXNG 指到本机的假服务）；再要一张链接
/// 卡片。代理一页都没交出去的报错：核心没走代理，这一段就没录到 HTML 换 Markdown 的那一路。
///
/// # Errors
///
/// 哪一步没成、代理没交出网页。
pub async fn fetch(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let mut rpc = head(ctx.sandbox).await?;
    ready(&mut rpc).await?;
    let session = open(ctx, &mut rpc, json!({})).await?;
    let calls = [
        (
            "web_fetch",
            json!({"url": "http://pages.perf.invalid/a.html"}),
        ),
        (
            "web_fetch",
            json!({"url": "http://pages.perf.invalid/a.html", "page": 2}),
        ),
        (
            "web_fetch",
            json!({"url": "http://docs.perf.invalid/b.html"}),
        ),
        ("web_search", json!({"query": "miyu function ordering"})),
    ];
    talk(ctx, &mut rpc, &session, "web", &calls).await?;
    ask(
        &mut rpc,
        "link.preview",
        json!({"url": "http://pages.perf.invalid/c.html"}),
    )
    .await?;
    if ctx.web.pages() < 2 {
        return Err(format!(
            "假代理只交出去 {} 页：核心没经代理抓网页",
            ctx.web.pages()
        ));
    }
    tokio::time::sleep(QUIET).await;
    Ok(())
}

/// 第九段：软件包和命令行。列、看几个包的说明和文件、核对、查文件归谁，关掉再打开一个内置包，预览、装上一个本机的包再卸掉，
/// 列技能、脚本，查升级、搜软件源；会话里模型查包；命令行 `miyu pkg list`、`miyu ask`、`miyu bus list`、`miyu check`。
/// 查升级、搜软件源要连网，连不上的只记一行、接着做（真用的人没网时也是这样）。
///
/// # Errors
///
/// 哪一步没成。
pub async fn packages(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let mut rpc = head(ctx.sandbox).await?;
    ready(&mut rpc).await?;
    for package in [
        "base-net",
        "base-knowledge",
        "base-memory",
        super::setup::TOOLS,
    ] {
        ask(&mut rpc, "package.info", json!({"package": package})).await?;
        ask(&mut rpc, "package.files", json!({"package": package})).await?;
    }
    ask(&mut rpc, "package.check", json!({"package": "base-net"})).await?;
    let manifest = ctx
        .sandbox
        .root
        .packages()
        .join(super::setup::TOOLS)
        .join("package.toml");
    ask(
        &mut rpc,
        "package.owns",
        json!({"path": manifest.to_string_lossy()}),
    )
    .await?;
    ask(
        &mut rpc,
        "package.disable",
        json!({"package": "base-mermaid"}),
    )
    .await?;
    ask(
        &mut rpc,
        "package.enable",
        json!({"package": "base-mermaid"}),
    )
    .await?;
    let local = ctx.placed.local.to_string_lossy().into_owned();
    ask(
        &mut rpc,
        "package.install",
        json!({"path": local, "preview": true}),
    )
    .await?;
    ask(&mut rpc, "package.install", json!({"path": local})).await?;
    ask(&mut rpc, "skill.list", json!({})).await?;
    ask(&mut rpc, "script.list", json!({})).await?;
    ask(&mut rpc, "package.remove", json!({"package": "perf-local"})).await?;
    for (method, params) in [
        ("package.upgrades", json!({})),
        ("package.search", json!({"query": "voice"})),
    ] {
        if let Err(e) = ask(&mut rpc, method, params).await {
            eprintln!("排函数：重场景里 {method} 没成（要连网）：{e}");
        }
    }
    let session = open(ctx, &mut rpc, json!({})).await?;
    let calls = [
        ("packages", json!({})),
        ("packages", json!({"package": "base-net"})),
        ("packages", json!({"search": "voice"})),
    ];
    talk(ctx, &mut rpc, &session, "packages", &calls).await?;
    cli(ctx, &["pkg", "list"])?;
    cli(ctx, &["ask", "perf-heavy-cli: one question."])?;
    cli(ctx, &["bus", "list"])?;
    // `miyu check` 照有没有要人看的问题定退出码：沙盒里的配置有几条提醒，只要它跑完。
    if let Err(e) = cli(ctx, &["check"]) {
        eprintln!("排函数：重场景里 miyu check 报了问题：{e}");
    }
    tokio::time::sleep(QUIET).await;
    Ok(())
}
