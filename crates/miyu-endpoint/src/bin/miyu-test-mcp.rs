//! 测试用的 MCP 服务（施工 X-3 上，`docs/blueprint/mcp.md`「守着它的」）：端点的测试把它写进 `bundle` 包的 `[[mcp]]`，让核心
//! 真起一个进程、经标准输入输出说 MCP。不随发行带出去。三个平台一样。
//!
//! 参数，照先后：
//!
//! - `legacy`：演旧时代（`server/discover` 回「没有这个方法」，等 `initialize` 握手）；不写的是新时代；
//! - `record:<路径>`：收到的每一行原样追加进这个文件，读到头了再记一行 `eof`；
//! - `changed`：第一次列完以后说一次工具变了，再列时多一件 `later`；
//! - `ttl:<毫秒>`：列的结果带 `ttlMs`（新时代），再列时多一件 `later`；
//! - `instructions:<字>`：探、握手的回应里交这段给模型的说明（施工 X-39）；
//! - `instructions-file:<路径>`：同上，起来的时候从这个文件读（没有的不交），测试改了文件、重启服务就换了说明；
//! - `prompts`：本事里声明 `prompts`（施工 X-39）：`review`（`code` 必填、`focus` 可选）回一句字、一张图、她说的一句；`hello` 不要
//!   参数，回一句；`two words` 和 `two_words` 清理以后撞名（后一个不登记）。缺必填的、不认识的名字回 `-32602`；
//! - `odd`：列的时候多几件登记不上的（施工 X-37）：没写参数格式的 `loose`、参数格式不是对象的 `stringly`、截到 64 个字以后
//!   撞名的 `long-x…x-one`、`long-x…x-two`（60 个 `x`），和一件没写名字的；
//! - `exit-if:<路径>`：这个路径在就一起来以退出码 3 退出（演缓存登记以后服务起不来）；
//! - `exit:<数>`：一起来就以这个退出码退出（演起不来、一直崩）；
//! - `stray:<字>`：一起来先往标准错误写一行。
//!
//! 几件工具：`echo` 照参数 `text` 回文字；`env` 回环境变量 `text` 的值（没有的是空的）；`cwd` 回工作目录；`picture` 回一张
//! 1×1 的 PNG 和一句话；`audio` 回一段声音；`link` 回一条链接；`fail` 说自己出错了；`structured` 只回结构化的；`huge` 回
//! 100005 个 `x`；`slow` 不回；`error` 回 JSON-RPC 的错；`garbled` 回认不出的 `resultType`；`die` 一收到就退出。只有 `echo`、
//! `env`、`cwd` 标了只读；只有 `echo` 写了给人看的名字 `Echo back`。

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};

use serde_json::{Value, json};

/// 1×1 的 PNG。
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

/// 列出来的几件。
const TOOLS: [&str; 13] = [
    "echo",
    "env",
    "cwd",
    "picture",
    "audio",
    "link",
    "fail",
    "structured",
    "huge",
    "slow",
    "error",
    "garbled",
    "die",
];

fn main() {
    let mut legacy = false;
    let mut record: Option<File> = None;
    let mut changed = false;
    let mut ttl: Option<u64> = None;
    let mut odd = false;
    let mut instructions: Option<String> = None;
    let mut prompts = false;
    for arg in std::env::args().skip(1) {
        if arg == "legacy" {
            legacy = true;
        } else if let Some(path) = arg.strip_prefix("record:") {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .expect("开得了记的文件");
            record = Some(file);
        } else if arg == "changed" {
            changed = true;
        } else if arg == "odd" {
            odd = true;
        } else if arg == "prompts" {
            prompts = true;
        } else if let Some(text) = arg.strip_prefix("instructions:") {
            instructions = Some(text.to_string());
        } else if let Some(path) = arg.strip_prefix("instructions-file:") {
            instructions = std::fs::read_to_string(path).ok();
        } else if let Some(ms) = arg.strip_prefix("ttl:") {
            ttl = Some(ms.parse().expect("毫秒是数"));
        } else if let Some(path) = arg.strip_prefix("exit-if:") {
            if std::path::Path::new(path).exists() {
                std::process::exit(3);
            }
        } else if let Some(code) = arg.strip_prefix("exit:") {
            std::process::exit(code.parse().expect("退出码是数"));
        } else if let Some(text) = arg.strip_prefix("stray:") {
            eprintln!("{text}");
        }
    }
    let stdin = io::stdin();
    let mut out = io::stdout();
    let mut lists = 0;
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            return;
        };
        if let Some(file) = record.as_mut() {
            writeln!(file, "{line}").expect("记得下");
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let (Some(method), Some(id)) = (message["method"].as_str(), message.get("id")) else {
            continue;
        };
        let answer = match method {
            "server/discover" if legacy => Err((-32601, "Method not found")),
            "server/discover" => Ok(json!({
                "resultType": "complete",
                "supportedVersions": ["2026-07-28"],
                "capabilities": capabilities(prompts),
                "instructions": instructions,
            })),
            "initialize" if legacy => Ok(json!({
                "protocolVersion": "2025-11-25",
                "capabilities": capabilities(prompts),
                "instructions": instructions,
                "serverInfo": {"name": "miyu-test-mcp", "version": "0"},
            })),
            "tools/list" => {
                lists += 1;
                let mut names: Vec<&str> = TOOLS.to_vec();
                if (changed || ttl.is_some()) && lists > 1 {
                    names.push("later");
                }
                let mut tools: Vec<Value> = names.iter().map(|name| tool(name)).collect();
                if odd {
                    tools.extend(odd_tools());
                }
                let mut result = json!({ "tools": tools });
                if let Some(ttl) = ttl {
                    result["ttlMs"] = json!(ttl);
                }
                Ok(result)
            }
            "tools/call" => match call(&message["params"]) {
                Some(result) => result,
                None => continue,
            },
            "prompts/list" if prompts => Ok(json!({"prompts": listed_prompts()})),
            "prompts/get" if prompts => prompt(&message["params"]),
            _ => Err((-32601, "Method not found")),
        };
        let reply = match answer {
            Ok(mut result) => {
                if !legacy && result.get("resultType").is_none() {
                    result["resultType"] = json!("complete");
                }
                json!({"jsonrpc": "2.0", "id": id, "result": result})
            }
            Err((code, text)) => {
                json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": text}})
            }
        };
        writeln!(out, "{reply}").expect("写得出去");
        if changed && method == "tools/list" && lists == 1 {
            let notice = json!({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"});
            writeln!(out, "{notice}").expect("写得出去");
        }
        out.flush().expect("写得出去");
    }
    // 读到头了：核心关了它的标准输入。
    if let Some(file) = record.as_mut() {
        writeln!(file, "eof").expect("记得下");
    }
}

/// 列出来的一件。
fn tool(name: &str) -> Value {
    let read_only = matches!(name, "echo" | "env" | "cwd");
    let mut tool = json!({
        "name": name,
        "description": format!("The {name} tool."),
        "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}},
        "annotations": {"readOnlyHint": read_only},
    });
    // 给人看的名字只写 `echo` 的（施工 9-8 六补）。
    if name == "echo" {
        tool["title"] = json!("Echo back");
    }
    tool
}

/// 声明的本事：工具，带了 `prompts` 的另有提示词（施工 X-39）。
fn capabilities(prompts: bool) -> Value {
    let mut capabilities = json!({"tools": {"listChanged": true}});
    if prompts {
        capabilities["prompts"] = json!({"listChanged": true});
    }
    capabilities
}

/// `prompts` 列的几个（施工 X-39）。
fn listed_prompts() -> Value {
    json!([
        {"name": "review", "title": "Review code", "description": "Review a piece of code.",
         "arguments": [{"name": "code", "required": true}, {"name": "focus"}]},
        {"name": "hello", "description": "Say hello."},
        {"name": "two words"},
        {"name": "two_words"},
    ])
}

/// `prompts/get`（施工 X-39）。
fn prompt(params: &Value) -> Result<Value, (i64, &'static str)> {
    let arguments = &params["arguments"];
    let said =
        |role: &str, text: String| json!({"role": role, "content": {"type": "text", "text": text}});
    let messages = match params["name"].as_str().unwrap_or_default() {
        "review" => {
            let Some(code) = arguments["code"].as_str() else {
                return Err((-32602, "Missing required argument: code"));
            };
            let focus = arguments["focus"]
                .as_str()
                .map(|focus| format!(" for {focus}"))
                .unwrap_or_default();
            json!([
                said("user", format!("Review this code{focus}:\n{code}")),
                {"role": "user", "content": {"type": "image", "data": PNG, "mimeType": "image/png"}},
                said("assistant", "Sure.".to_string()),
            ])
        }
        "hello" => json!([said("user", "Say hello.".to_string())]),
        "two words" => json!([said("user", "Two.".to_string())]),
        _ => return Err((-32602, "Unknown prompt")),
    };
    Ok(json!({"messages": messages}))
}

/// `odd` 多列的几件（施工 X-37）：都登记不上。
fn odd_tools() -> Vec<Value> {
    let long = |end: &str| format!("long-{}-{end}", "x".repeat(60));
    let object = json!({"type": "object"});
    vec![
        json!({"name": "loose"}),
        json!({"name": "stringly", "inputSchema": {"type": "string"}}),
        json!({"name": long("one"), "inputSchema": object}),
        json!({"name": long("two"), "inputSchema": object}),
        json!({"description": "no name", "inputSchema": object}),
    ]
}

/// 调一件：交回结果或错误；不回的（`slow`）没有。`die` 当场退出。
fn call(params: &Value) -> Option<Result<Value, (i64, &'static str)>> {
    let said = params["arguments"]["text"].as_str().unwrap_or_default();
    let text = |text: &str| json!({"content": [{"type": "text", "text": text}]});
    let result = match params["name"].as_str().unwrap_or_default() {
        "echo" | "later" => text(said),
        "env" => text(&std::env::var(said).unwrap_or_default()),
        "cwd" => text(
            &std::env::current_dir()
                .map(|dir| dir.display().to_string())
                .unwrap_or_default(),
        ),
        "picture" => json!({"content": [
            {"type": "image", "data": PNG, "mimeType": "image/png"},
            {"type": "text", "text": "a picture"},
        ]}),
        "audio" => {
            json!({"content": [{"type": "audio", "data": "aGVsbG8=", "mimeType": "audio/wav"}]})
        }
        "link" => {
            json!({"content": [{"type": "resource_link", "uri": "file:///notes.txt", "name": "notes"}]})
        }
        "fail" => json!({"content": [{"type": "text", "text": "broke"}], "isError": true}),
        "structured" => json!({"content": [], "structuredContent": {"answer": 42}}),
        "huge" => text(&"x".repeat(100_005)),
        "slow" => return None,
        "error" => return Some(Err((-32000, "server says no"))),
        "garbled" => json!({"resultType": "weird"}),
        "die" => std::process::exit(0),
        _ => return Some(Err((-32602, "Unknown tool"))),
    };
    Some(Ok(result))
}
