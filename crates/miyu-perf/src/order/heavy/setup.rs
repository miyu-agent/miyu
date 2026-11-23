//! 重场景的布置（施工 LW-30）：往沙盒的数据根装两个扩展（一个是平台桥）、批它们的能力、打开它们；系统配置加三个 MCP 服务、
//! SearXNG、主人对应表、远程的语义模型；知识库要加的几份文件、要挂载的目录；一个本机的软件包文件夹（装、卸那一步用）。扩展和命令型的 MCP 服务
//! 都是拷过去的量尺本身（`heavy/role.rs`、`heavy/mcp.rs`）。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{mcp, plans, role};
use crate::sandbox::Sandbox;

/// 扩展的包编号。
pub const TOOLS: &str = "perf-tools";

/// 平台桥的包编号。
pub const BRIDGE: &str = "perf-bridge";

/// 扩展声明的能力：照写扩展的说明里各样本事要的。
const TOOLS_CAPS: [&str; 13] = [
    "commands",
    "context.inject",
    "context.system",
    "events.read",
    "events.write",
    "fs.read",
    "memory.write",
    "network",
    "search",
    "sessions.drive",
    "tool.guard",
    "tools",
    "widgets",
];

/// 平台桥声明的能力：同接入 QQ 的桥经扩展连接用得到的那几项。
const BRIDGE_CAPS: [&str; 5] = [
    "act_for_external",
    "events.read",
    "events.write",
    "sessions.drive",
    "tools",
];

/// 扩展清单末尾接上的：一项配置、一个预设功能、斜杠命令、搜索来源、守卫、调用后的通知、两块界面块。
const TOOLS_TABLES: &str = r#"
[settings.greeting]
type = "text"
max = 40
default = "Hello"
layers = ["system"]
applies = "now"
name = { en = "Greeting", zh = "问候语" }

[features.perf-heavy]
name = { en = "Heavy", zh = "重场景" }
tools = ["perf_lookup", "perf_note"]

[[slash]]
name = "perf-heavy"
summary = { en = "Heavy command", zh = "重场景命令" }
access = "anyone"

[[search]]
id = "find"
name = { en = "Heavy search", zh = "重场景搜索" }

[guard]
tools = ["write"]
access = ["execute"]

[after]
tools = ["shell", "perf_lookup"]
access = ["write"]

[[widget]]
id = "status"
place = "status"

[[widget]]
id = "panel"
place = "sidebar"
title = { en = "Heavy", zh = "重场景" }
"#;

/// 平台桥清单末尾接上的：平台、判官的旁路请求和它的用途名。
const BRIDGE_TABLES: &str = r#"
[connection]
platform = "qq"

[[side]]
id = "judge"
tier = "lite"
name = { en = "Reply judge", zh = "判断要不要回" }
summary = { en = "Decides whether to reply in a group.", zh = "群里有人说话时判断要不要回" }

[[purpose]]
id = "judge"
name = { en = "Reply judge", zh = "群聊判断" }
"#;

/// 布置好的几样东西在哪。
pub struct Placed {
    /// 控制目录：量尺往里放文件，扮扩展的照它往下走（`pause`、`once`）。
    pub ctl: PathBuf,
    /// 扩展的记录（`heavy/role.rs`）。
    pub tools_record: PathBuf,
    /// 平台桥的记录。
    pub bridge_record: PathBuf,
    /// 知识库要加的几份文件所在的目录。
    pub documents: PathBuf,
    /// 知识库要挂载的目录。
    pub mount: PathBuf,
    /// 本机的软件包文件夹。
    pub local: PathBuf,
    /// 拷过去当命令型 MCP 服务的量尺。
    pub server: PathBuf,
}

/// 布置（见模块开头）：`dir` 是重场景自己的目录，`port` 是假服务的口（`heavy/web.rs`）。核心起来以前调。
///
/// # Errors
///
/// 建不了、写不进、找不到或者拷不了量尺自己、系统配置读不了。
pub fn place(sandbox: &Sandbox, dir: &Path, port: u16) -> Result<Placed, String> {
    let ctl = dir.join("ctl");
    let documents = dir.join("documents");
    let mount = dir.join("mount");
    let local = dir.join("packages").join("perf-local");
    for d in [
        &ctl,
        &documents,
        &mount,
        &local.join("skills").join("tidy"),
        &dir.join("bin"),
    ] {
        std::fs::create_dir_all(d).map_err(|e| format!("建不了 {}：{e}", d.display()))?;
    }
    let me = std::env::current_exe().map_err(|e| format!("找不到量尺自己：{e}"))?;
    let tools_record = extension(
        sandbox,
        dir,
        &me,
        (TOOLS, &TOOLS_CAPS),
        &plans::extension(),
        TOOLS_TABLES,
    )?;
    let bridge_record = extension(
        sandbox,
        dir,
        &me,
        (BRIDGE, &BRIDGE_CAPS),
        &plans::bridge(),
        BRIDGE_TABLES,
    )?;
    let approved = json!({TOOLS: TOOLS_CAPS, BRIDGE: BRIDGE_CAPS});
    let switches = json!({"version": 1, "on": {TOOLS: true, BRIDGE: true}, "approved": approved});
    write(&sandbox.root.extensions_file(), &format!("{switches}\n"))?;
    let server = dir
        .join("bin")
        .join(format!("perf-mcp{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(&me, &server).map_err(|e| format!("拷不了量尺自己：{e}"))?;
    let config = sandbox.root.config_file();
    let mut text = std::fs::read_to_string(&config).map_err(|e| format!("系统配置读不了：{e}"))?;
    // 远程的语义模型：假模型照 `fake/embed.rs` 回向量，记忆、知识库照它算（真用的人多半开着语义模型）。
    let chat = "[models]\nchat = \"perf/perf-model\"\n";
    if !text.contains(chat) {
        return Err(format!(
            "系统配置里没有 {chat:?}：沙盒的配置换了写法，这里跟着改"
        ));
    }
    text = text.replace(chat, &format!("{chat}embedding = \"perf/perf-embed\"\n"));
    text.push_str(&format!(
        "\n[net.search]\nsource = \"searxng\"\n\n[net.search.searxng]\nurl = \"http://127.0.0.1:{port}\"\n\n\
[external.bindings]\n\"qq:10001\" = \"owner\"\n\n[mcp.perfstdio]\ncommand = [{}, \"{}\"]\n\n\
[mcp.perfhttp]\nurl = \"http://127.0.0.1:{port}/mcp\"\n\n[mcp.perfauth]\nurl = \"http://127.0.0.1:{port}/mcp2\"\n",
        toml_string(&server), mcp::ARG
    ));
    write(&config, &text)?;
    write(
        &documents.join("notes.md"),
        &"# 排函数\n\n排函数把执行过的函数排在代码段最前面。\n\n## 清单\n\n清单由量尺照剧本录。\n"
            .repeat(30),
    )?;
    write(
        &documents.join("guide.txt"),
        &"Heavy scenario guide.\n".repeat(200),
    )?;
    write(&documents.join("page.html"), &super::web::page("kb"))?;
    std::fs::write(
        documents.join("small.pdf"),
        pdf("Function ordering keeps hot code together."),
    )
    .map_err(|e| format!("写不进 PDF：{e}"))?;
    for n in 0..6 {
        write(
            &mount.join(format!("note-{n}.md")),
            &format!(
                "# 第 {n} 篇\n\n{}",
                "蓝图里写着记忆、会话和知识库怎么走。\n\n".repeat(40)
            ),
        )?;
    }
    write(
        &local.join("package.toml"),
        "[package]\nversion = \"0.1.0\"\nname = { en = \"Heavy local\", zh = \"本地包\" }\nsummary = { en = \"A local package\", zh = \"本地包\" }\n",
    )?;
    write(
        &local.join("skills").join("tidy").join("SKILL.md"),
        "---\nname: tidy\ndescription: Tidy up a folder.\n---\n\nTidy the folder step by step.\n",
    )?;
    Ok(Placed {
        ctl,
        tools_record,
        bridge_record,
        documents,
        mount,
        local,
        server,
    })
}

/// 装一个量尺自己当程序的扩展包：编号 `id`、能力 `caps`、剧本 `plan`，清单末尾接上 `tables`。开机就起（`start = "always"`）。
/// 交回它的记录在哪。
fn extension(
    sandbox: &Sandbox,
    dir: &Path,
    me: &Path,
    (id, caps): (&str, &[&str]),
    plan: &Value,
    tables: &str,
) -> Result<PathBuf, String> {
    let package = sandbox.root.packages().join(id);
    let bin = package.join("bin");
    std::fs::create_dir_all(&bin).map_err(|e| format!("建不了 {}：{e}", bin.display()))?;
    std::fs::copy(
        me,
        bin.join(format!("{id}{}", std::env::consts::EXE_SUFFIX)),
    )
    .map_err(|e| format!("拷不了量尺自己：{e}"))?;
    let plan_file = dir.join(format!("{id}.plan.json"));
    write(&plan_file, &plan.to_string())?;
    let record = dir.join(format!("{id}.record.jsonl"));
    let args = [
        Value::String(role::ARG.to_string()).to_string(),
        toml_string(&plan_file),
        toml_string(&dir.join("ctl")),
        toml_string(&record),
    ];
    let caps: Vec<String> = caps.iter().map(|c| format!("\"{c}\"")).collect();
    let manifest = format!(
        "[package]\nversion = \"0.1.0\"\nprotocol = [1, 1]\nname = {{ en = \"{id}\", zh = \"{id}\" }}\nsummary = {{ en = \"Heavy scenario\", zh = \"重场景\" }}\n\n\
[command]\nname = \"{id}\"\nprogram = \"{id}\"\nabout = {{ en = \"Heavy\", zh = \"重场景\" }}\n\n\
[process]\nargs = [{}]\nstart = \"always\"\ncapabilities = [{}]\n{tables}",
        args.join(", "),
        caps.join(", ")
    );
    write(&package.join("package.toml"), &manifest)?;
    Ok(record)
}

/// 路径写成 TOML 的一个字：用 JSON 的写法（TOML 的基本字符串认同样的转义）。
fn toml_string(path: &Path) -> String {
    Value::String(path.to_string_lossy().into_owned()).to_string()
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("写不进 {}：{e}", path.display()))
}

/// 一页只有一行字的 PDF：知识库加 PDF 时核心起 `miyu-extract` 读它。
pub fn pdf(line: &str) -> Vec<u8> {
    let stream = format!("BT /F1 12 Tf 20 100 Td ({line}) Tj ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 144] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut out = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (n, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.push_str(&format!("{} 0 obj\n{object}\nendobj\n", n + 1));
    }
    let xref = out.len();
    out.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objects.len() + 1
    ));
    for offset in offsets {
        out.push_str(&format!("{offset:010} 00000 n \n"));
    }
    out.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    ));
    out.into_bytes()
}
