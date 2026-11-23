//! 重场景的前四段（施工 LW-30）：扩展和平台桥、多会话、网页界面那样的头加用量和模型、知识库。每段都是核心刚起来、
//! 像头那样连上问一遍（`order/script.rs` 的 `head`）以后接着做；做完歇 `QUIET`。

use serde_json::json;

use super::setup::{BRIDGE, TOOLS};
use super::steps::{
    BRIDGE_SAYS, Ctx, ask, cli, extension_is, finished, open, release, script, servers_running,
    talk, until,
};
use crate::order::script::{QUIET, head};
use crate::rpc::Rpc;

/// 扩展和 MCP 服务都在跑：核心起来时照开关拉起（扩展开机就起，MCP 服务在后台起），每一段开头先等它们。
///
/// # Errors
///
/// 等太久。
pub async fn ready(rpc: &mut Rpc) -> Result<(), String> {
    until(
        rpc,
        "两个扩展在跑",
        "extension.status",
        json!({}),
        |s| extension_is(s, TOOLS, "running") && extension_is(s, BRIDGE, "running"),
    )
    .await?;
    until(rpc, "两个 MCP 服务在跑", "mcp.list", json!({}), |l| {
        servers_running(l, &["perfstdio", "perfhttp"])
    })
    .await?;
    Ok(())
}

/// 第一段：扩展和平台桥。
///
/// 放行扩展（它自己开会话说一句，读读写写，见 `heavy/plans.rs`），假模型照它那一句回查找、记一笔、跑一条命令（守卫问它、
/// `tool.after` 通知它）；量尺这一头在会话里调它的工具、斜杠命令、后台页的方法，问端口、预设；重启、关掉、打开它，再说一句。
/// 再放行平台桥（三个群、两个私聊、判官、`venue.records`），等它做完，看几个会话的视图。
///
/// # Errors
///
/// 哪一步没成。
pub async fn extensions(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let mut rpc = head(ctx.sandbox).await?;
    ready(&mut rpc).await?;
    ask(&mut rpc, "subscribe", json!({"stream": "extensions"})).await?;
    let says = [
        ("perf_lookup", json!({"query": "alpha"})),
        ("perf_note", json!({"text": "n1"})),
        (
            "shell",
            json!({"command": "echo perf", "description": "step"}),
        ),
    ];
    ctx.fake
        .script(Some(script("perf-heavy-extension-says", &says)));
    release(ctx.placed, "go-extension")?;
    let done = finished(&ctx.placed.tools_record, 1).await;
    ctx.fake.script(None);
    done?;
    let session = open(ctx, &mut rpc, json!({})).await?;
    talk(ctx, &mut rpc, &session, "extension", &says).await?;
    ask(
        &mut rpc,
        "command.run",
        json!({"session": session, "text": "/perf-heavy hello there"}),
    )
    .await?;
    ask(
        &mut rpc,
        "package.call",
        json!({"package": TOOLS, "method": "status", "params": {}}),
    )
    .await?;
    ask(&mut rpc, "port.list", json!({})).await?;
    ask(&mut rpc, "preset.get", json!({"preset": "full"})).await?;
    ask(&mut rpc, "extension.restart", json!({"package": TOOLS})).await?;
    until(
        &mut rpc,
        "扩展重启好",
        "extension.status",
        json!({}),
        |s| extension_is(s, TOOLS, "running"),
    )
    .await?;
    ask(&mut rpc, "extension.disable", json!({"package": TOOLS})).await?;
    until(
        &mut rpc,
        "扩展关掉",
        "extension.status",
        json!({}),
        |s| extension_is(s, TOOLS, "off"),
    )
    .await?;
    ask(&mut rpc, "extension.enable", json!({"package": TOOLS})).await?;
    until(
        &mut rpc,
        "扩展又在跑",
        "extension.status",
        json!({}),
        |s| extension_is(s, TOOLS, "running"),
    )
    .await?;
    ask(&mut rpc, "package.info", json!({"package": TOOLS})).await?;
    talk(ctx, &mut rpc, &session, "extension-again", &says[..1]).await?;
    ctx.fake.script(Some(script(
        BRIDGE_SAYS,
        &[("perf_lookup", json!({"query": "排函数"}))],
    )));
    release(ctx.placed, "go-bridge")?;
    let done = finished(&ctx.placed.bridge_record, 1).await;
    ctx.fake.script(None);
    done?;
    let listed = ask(&mut rpc, "session.list", json!({})).await?;
    for one in listed["sessions"].as_array().into_iter().flatten().take(3) {
        ask(&mut rpc, "view.page", json!({"session": one["session"]})).await?;
    }
    tokio::time::sleep(QUIET).await;
    Ok(())
}

/// 第二段：多会话。开十个会话、五个说话，派一次子代理，给另一个会话发一句；改标题、置顶、分叉、回顾、两种导出、用量、
/// 上下文、额度、提示词；列一遍，删一个。
///
/// # Errors
///
/// 哪一步没成。
pub async fn sessions(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let mut rpc = head(ctx.sandbox).await?;
    ready(&mut rpc).await?;
    let mut made = Vec::new();
    for _ in 0..10 {
        made.push(open(ctx, &mut rpc, json!({})).await?);
    }
    for session in &made[..5] {
        talk(ctx, &mut rpc, session, "chat", &[]).await?;
    }
    let spawn = [(
        "subagent",
        json!({"description": "look around", "prompt": "Read small files and report."}),
    )];
    talk(ctx, &mut rpc, &made[0], "spawn", &spawn).await?;
    let peer = [
        ("sessions", json!({})),
        (
            "send_message",
            json!({"to": made[2], "message": "hello from a peer"}),
        ),
    ];
    talk(ctx, &mut rpc, &made[1], "peer", &peer).await?;
    let first = &made[0];
    ask(
        &mut rpc,
        "session.retitle",
        json!({"session": first, "title": "重场景"}),
    )
    .await?;
    ask(
        &mut rpc,
        "session.set_meta",
        json!({"session": first, "pinned": true}),
    )
    .await?;
    ask(&mut rpc, "session.fork", json!({"session": first})).await?;
    ask(&mut rpc, "session.recap", json!({"session": first})).await?;
    ask(
        &mut rpc,
        "session.export",
        json!({"session": first, "format": "markdown", "to": "inline"}),
    )
    .await?;
    let bundle = ctx.sandbox.work.join("exported.miyu");
    ask(
        &mut rpc,
        "session.export",
        json!({"session": first, "format": "bundle", "to": {"path": bundle.to_string_lossy()}}),
    )
    .await?;
    for method in [
        "session.usage_parts",
        "session.context",
        "session.quota",
        "session.prompts",
    ] {
        ask(&mut rpc, method, json!({"session": first})).await?;
    }
    ask(&mut rpc, "session.list", json!({"dream": false})).await?;
    ask(&mut rpc, "session.delete", json!({"session": made[9]})).await?;
    tokio::time::sleep(QUIET).await;
    Ok(())
}

/// 网页界面订阅的几条流（照 `miyu-web` 连上以后订的）。
const WEB_STREAMS: [&str; 10] = [
    "sessions",
    "config",
    "extensions",
    "packages",
    "projects",
    "bus",
    "schedules",
    "notices",
    "kb",
    "widgets",
];

/// 第三段：照网页界面的样子连上（`head.kind` 是 `web`、能让人输入、前缀要变时先问），订它订的流、问它问的几样，看几个会话的
/// 视图，开一个会话说一句；再查用量的几种分组、换算，刷新模型列表，问供应商，换个模型说一句。
///
/// # Errors
///
/// 哪一步没成。
pub async fn web(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let mut rpc = Rpc::connect_as(
        &ctx.sandbox.root,
        "web",
        json!({"input": true, "prefix_ask": true}),
    )
    .await?;
    ready(&mut rpc).await?;
    for stream in WEB_STREAMS {
        ask(&mut rpc, "subscribe", json!({"stream": stream})).await?;
    }
    let persona = "software-engineer";
    ask(
        &mut rpc,
        "subscribe",
        json!({"stream": "memory", "persona": persona}),
    )
    .await?;
    ask(&mut rpc, "notices.claim", json!({})).await?;
    let asks = [
        ("config.schema", json!({})),
        ("config.get", json!({})),
        ("command.catalog", json!({})),
        ("model.list", json!({})),
        ("persona.list", json!({})),
        ("persona.get", json!({"persona": persona})),
        (
            "persona.read",
            json!({"persona": persona, "prompt": "persona"}),
        ),
        ("preset.list", json!({})),
        ("preset.get", json!({"preset": "full"})),
        ("skill.list", json!({})),
        ("script.list", json!({})),
        ("human.get", json!({})),
        ("project.list", json!({})),
        ("usage.query", json!({"group": ["day"]})),
        ("memory.list", json!({"persona": persona})),
        ("schedule.list", json!({})),
        ("kb.list", json!({})),
        ("mcp.list", json!({})),
        ("package.list", json!({})),
        ("extension.status", json!({})),
        ("bus.list", json!({})),
        ("secret.list", json!({})),
        ("provider.catalog", json!({"featured": true})),
    ];
    for (method, params) in asks {
        ask(&mut rpc, method, params).await?;
    }
    ask(
        &mut rpc,
        "project.create",
        json!({"name": "重场景", "workspace": ctx.sandbox.work.to_string_lossy()}),
    )
    .await?;
    let listed = ask(&mut rpc, "session.list", json!({"dream": false})).await?;
    for one in listed["sessions"].as_array().into_iter().flatten().take(4) {
        let session = &one["session"];
        ask(
            &mut rpc,
            "subscribe",
            json!({"stream": "view", "session": session}),
        )
        .await?;
        ask(&mut rpc, "view.page", json!({"session": session})).await?;
        ask(&mut rpc, "session.viewing", json!({"session": session})).await?;
        ask(&mut rpc, "session.usage_parts", json!({"session": session})).await?;
    }
    let session = open(ctx, &mut rpc, json!({})).await?;
    talk(ctx, &mut rpc, &session, "web-head", &[]).await?;
    ask(
        &mut rpc,
        "subscribe",
        json!({"stream": "view", "session": session}),
    )
    .await?;
    ask(&mut rpc, "view.page", json!({"session": session})).await?;
    ask(
        &mut rpc,
        "session.configure",
        json!({"session": session, "context": 100_000}),
    )
    .await?;
    let cwd = ctx.sandbox.work.to_string_lossy().into_owned();
    ask(&mut rpc, "fs.list", json!({"cwd": cwd, "dir": ""})).await?;
    ask(
        &mut rpc,
        "fs.find",
        json!({"cwd": cwd, "query": "exported", "fresh": true, "wait": true}),
    )
    .await?;
    ask(
        &mut rpc,
        "config.set",
        json!({"changes": [{"key": "ui.language", "value": "zh"}]}),
    )
    .await?;
    for group in [
        json!(["model"]),
        json!(["session"]),
        json!(["purpose"]),
        json!(["use"]),
        json!(["venue", "model"]),
    ] {
        ask(&mut rpc, "usage.query", json!({"group": group})).await?;
    }
    ask(&mut rpc, "usage.query", json!({})).await?;
    ask(
        &mut rpc,
        "usage.convert",
        json!({"groups": [[{"currency": "USD", "amount": 1.0, "day": "2026-11-20"}]]}),
    )
    .await?;
    ask(&mut rpc, "model.list", json!({"refresh": true})).await?;
    ask(&mut rpc, "model.list", json!({"provider": "perf"})).await?;
    ask(&mut rpc, "provider.catalog", json!({"query": "deep"})).await?;
    ask(&mut rpc, "provider.detect", json!({})).await?;
    ask(
        &mut rpc,
        "provider.test",
        json!({"provider": "perf", "model": "perf-model"}),
    )
    .await?;
    let other = open(ctx, &mut rpc, json!({})).await?;
    ask(
        &mut rpc,
        "session.configure",
        json!({"session": other, "model": "perftls/perf-model"}),
    )
    .await?;
    talk(ctx, &mut rpc, &other, "other-model", &[]).await?;
    ask(&mut rpc, "session.usage_parts", json!({"session": other})).await?;
    tokio::time::sleep(QUIET).await;
    Ok(())
}

/// 第四段：知识库。建一个库、加四种文件（md、txt、html、pdf，后两种起 `miyu-extract`），挂一个只读的目录，等索引建好；
/// 列文档、搜、读，打开自动带出，在会话里说两句（模型调 `kb_search`、`kb_read`）；命令行 `miyu kb list`、`miyu kb search`。
///
/// # Errors
///
/// 哪一步没成。
pub async fn knowledge(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let mut rpc = head(ctx.sandbox).await?;
    ready(&mut rpc).await?;
    ask(&mut rpc, "subscribe", json!({"stream": "kb"})).await?;
    ask(
        &mut rpc,
        "kb.create",
        json!({"kb": "notes", "name": "笔记", "description": "排函数的笔记"}),
    )
    .await?;
    for file in ["notes.md", "guide.txt", "page.html", "small.pdf"] {
        let path = ctx.placed.documents.join(file);
        let put = ask(
            &mut rpc,
            "blob.put",
            json!({"path": path.to_string_lossy()}),
        )
        .await?;
        ask(
            &mut rpc,
            "kb.add",
            json!({"kb": "notes", "path": file, "blob": put["blob"]}),
        )
        .await?;
    }
    ask(
        &mut rpc,
        "kb.mount",
        json!({"kb": "mounted", "folder": ctx.placed.mount.to_string_lossy(), "name": "蓝图"}),
    )
    .await?;
    until(
        &mut rpc,
        "两个库的索引建好",
        "kb.list",
        json!({}),
        |l| {
            l["bases"].as_array().is_some_and(|all| {
                all.len() >= 2
                    && all.iter().all(|b| {
                        b["indexing"] != true && b["chunks"].as_u64().is_some_and(|n| n > 0)
                    })
            })
        },
    )
    .await?;
    for kb in ["notes", "mounted"] {
        ask(&mut rpc, "kb.documents", json!({"kb": kb})).await?;
    }
    for query in ["排函数", "清单", "function ordering", "蓝图 记忆"] {
        ask(&mut rpc, "kb.search", json!({"query": query})).await?;
    }
    for path in ["small.pdf", "page.html"] {
        ask(&mut rpc, "kb.read", json!({"kb": "notes", "path": path})).await?;
    }
    ask(&mut rpc, "kb.update", json!({"kb": "notes", "auto": true})).await?;
    let session = open(ctx, &mut rpc, json!({})).await?;
    let looks = [
        ("kb_search", json!({"query": "排函数 清单"})),
        ("kb_read", json!({"kb": "notes", "path": "notes.md"})),
        ("kb_search", json!({"query": "蓝图", "kb": "mounted"})),
    ];
    talk(ctx, &mut rpc, &session, "kb-look", &looks).await?;
    talk(ctx, &mut rpc, &session, "kb-again", &[]).await?;
    cli(ctx, &["kb", "list"])?;
    cli(ctx, &["kb", "search", "排函数"])?;
    tokio::time::sleep(QUIET).await;
    Ok(())
}
