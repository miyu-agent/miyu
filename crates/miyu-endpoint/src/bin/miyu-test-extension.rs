//! 测试用的扩展（施工 9-4 上，`docs/blueprint/extensions.md`「守着它的」）：端点的测试把它放在测试程序旁边，当成
//! `process` 包的程序让核心拉起。不随发行带出去。只用标准库，三个平台一样。
//!
//! 参数是一步步要做的，照先后：
//!
//! - `record:<路径>`：往这个文件追加记下的（先记一行 `cwd:<工作目录>`）；
//! - `err:<字>`：往标准错误写一行；
//! - `env:<名字>`：记下一行 `<名字>=<这个环境变量的值>`，没有的值是空的（施工 O-18）；
//! - `exists:<路径>`：记下一行 `exists:true` 或 `exists:false`，这时这个路径在不在（施工 F-5 补：卸包先停再删）；
//! - `hello`：发握手（不带凭据），读一行回应记下；
//! - `call:<方法>`：发一条不带参数的请求，读一行回应记下；
//! - `ask:<方法>:<JSON 参数>`：发一条带参数的请求，读一行回应记下；参数里的 `{session}` 换成最近一条带 `"session"` 的回应里
//!   的会话编号（施工 O-4 下：造了场所会话接着对它说话）。等回应时先来的 `widget.watch` 照 `serve` 答、接着等，`widget.set`
//!   的回应记下接着等（施工 X-15 中：握了手核心就会说谁在看）；
//! - `wait`：读标准输入直到读到头；
//! - `listen`：读标准输入直到读到头，读到的每一行都记下（施工 9-4 下下：核心推来的 `extension.config`）；
//! - `serve`：读标准输入直到读到头，每一行都记下；是核心发来的 `tool.call` 的，照工具名答（施工 O-2 上）：`boom` 回一个错误，
//!   `garbled` 回写法不对的结果，`oops` 回一段字、说出了错，别的回一段字 `served <工具名>`；是 `method.call` 的照方法名答
//!   （施工 F-6 中）：`boom` 回一个错误，`slow` 不答，别的回 `{"answered": <方法名>}`；是 `command.call` 的照 `argument` 答
//!   （施工 X-14 下）：`boom` 回一个错误，`garbled` 回写法不对的结果，`silent` 不答，别的回两行 `{"said": "<命令> <argument>\nok"}`；
//!   是 `widget.watch` 的推一块 `widget.set`：一段字 `watching <块>`，跟着会话的后面接会话编号（施工 X-15 中）；`method.call` 的
//!   `burst` 先给块 `temp` 连推十次 `n=0` 到 `n=9`，再答；`quit` 答了就以退出码 0 退出；是 `user.append` 的照 `inject` 定的答
//!   （施工 X-16）；是 `search.query` 的照 `query` 答（施工 N-5）：`limit` 回限额的错误，`boom` 回一个错误，别的答一条；
//! - `inject:<怎么答>`：之后的 `serve` 照它答 `user.append`：`hint` 交一块 `meme`，字是 `hint <venue> <text>`（没有 `text` 的写
//!   `-`）；`slow` 过 600 毫秒再照 `hint` 答；`silent` 不答；`boom` 回一个错误；`garbled` 回写法不对的；`many` 交四块；`empty` 交空的。不写的不答；
//! - `sent:<blob>`：之后 `serve` 答工具 `sticker` 时，`sent` 里放这个 blob 的一张图（施工 X-19）；
//! - `give:<字,字>`：之后 `serve` 答 `method.call` 的 `give` 时回 `{"given": [这几段字]}`（施工 W-6 三补：回应里带 blob）；
//! - `pause:<路径>`：等到这个路径有了再往下走（施工 X-20：测试照会话说完了几轮放它走）；
//! - `hang`：不管标准输入，一直睡；
//! - `exit:<数>`：以这个退出码退出。
//!
//! 照 `#!` 起的（包里的程序是一个脚本、第一行 `#!` 指着它，施工 X-34），第一个参数是脚本本身：是一个在的文件的跳过。
//!
//! 做完了以退出码 0 退出。

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::time::Duration;

fn main() {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut record: Option<File> = None;
    // 最近一条带会话编号的回应里的那个编号：`ask` 的参数照它换。
    let mut session = String::new();
    // `serve` 怎么答 `user.append`（施工 X-16）。
    let mut inject = String::from("silent");
    // `serve` 答工具 `sticker` 时发出去的那张图的 blob（施工 X-19）。
    let mut sent = String::new();
    // `serve` 答 `method.call` 的 `give` 时交回的几段字（施工 W-6 三补）。
    let mut give = String::new();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let script = usize::from(
        args.first()
            .is_some_and(|first| std::path::Path::new(first).is_file()),
    );
    for (n, step) in args.into_iter().skip(script).enumerate() {
        if let Some(path) = step.strip_prefix("record:") {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .expect("开得了记的文件");
            let cwd = std::env::current_dir().expect("有工作目录");
            writeln!(file, "cwd:{}", cwd.display()).expect("写得进");
            record = Some(file);
        } else if let Some(text) = step.strip_prefix("err:") {
            // 整行一次写（test-ext 三补）：`eprintln!` 分两次写字和换行，核心退避时杀进程正好落在中间，Windows 上就少一个换行。
            let line = format!("{text}\n");
            io::stderr().write_all(line.as_bytes()).expect("写得进");
        } else if let Some(name) = step.strip_prefix("env:") {
            let value = std::env::var_os(name).unwrap_or_default();
            keep(&mut record, &format!("{name}={}", value.to_string_lossy()));
        } else if let Some(path) = step.strip_prefix("exists:") {
            let there = std::path::Path::new(path).exists();
            keep(&mut record, &format!("exists:{there}"));
        } else if step == "hello" {
            send(&format!(
                r#"{{"jsonrpc":"2.0","id":"s{n}","method":"hello","params":{{"protocol":[1,1],"head":{{"kind":"test-extension","version":"0"}}}}}}"#
            ));
            keep(&mut record, &read(&mut input));
        } else if let Some(method) = step.strip_prefix("call:") {
            send(&format!(
                r#"{{"jsonrpc":"2.0","id":"s{n}","method":"{method}","params":{{}}}}"#
            ));
            keep(&mut record, &read(&mut input));
        } else if let Some(rest) = step.strip_prefix("ask:") {
            let (method, params) = rest.split_once(':').expect("写成 ask:<方法>:<JSON 参数>");
            let params = params.replace("{session}", &session);
            send(&format!(
                r#"{{"jsonrpc":"2.0","id":"s{n}","method":"{method}","params":{params}}}"#
            ));
            let mut reply = read(&mut input);
            while reply.contains(r#""method":"widget.watch""#) || reply.contains(r#""id":"w-"#) {
                keep(&mut record, &reply);
                if let Some(widget) =
                    field(&reply, "widget").filter(|_| reply.contains("widget.watch"))
                {
                    send(&shown(
                        &widget,
                        field(&reply, "session").as_deref(),
                        "watching",
                    ));
                }
                reply = read(&mut input);
            }
            if let Some(id) = session_in(&reply) {
                session = id;
            }
            keep(&mut record, &reply);
        } else if let Some(blob) = step.strip_prefix("sent:") {
            blob.clone_into(&mut sent);
        } else if let Some(given) = step.strip_prefix("give:") {
            given.clone_into(&mut give);
        } else if let Some(how) = step.strip_prefix("inject:") {
            how.clone_into(&mut inject);
        } else if step == "wait" {
            while !read(&mut input).is_empty() {}
        } else if step == "listen" {
            loop {
                let line = read(&mut input);
                if line.is_empty() {
                    break;
                }
                keep(&mut record, &line);
            }
        } else if step == "serve" {
            loop {
                let line = read(&mut input);
                if line.is_empty() {
                    break;
                }
                keep(&mut record, &line);
                if line.contains(r#""method":"tool.call""#)
                    && let Some(id) = field(&line, "id")
                    && let Some(answer) =
                        served(&id, &field(&line, "tool").unwrap_or_default(), &sent)
                {
                    send(&answer);
                }
                if line.contains(r#""method":"widget.watch""#)
                    && let Some(widget) = field(&line, "widget")
                {
                    send(&shown(
                        &widget,
                        field(&line, "session").as_deref(),
                        "watching",
                    ));
                }
                if line.contains(r#""method":"method.call""#)
                    && inner_method(&line).as_deref() == Some("burst")
                {
                    for n in 0..10 {
                        send(&shown("temp", None, &format!("n={n}")));
                    }
                }
                if line.contains(r#""method":"method.call""#)
                    && let Some(id) = field(&line, "id")
                    && let Some(answer) =
                        answered(&id, &inner_method(&line).unwrap_or_default(), &give)
                {
                    send(&answer);
                    if inner_method(&line).as_deref() == Some("quit") {
                        std::process::exit(0);
                    }
                }
                if line.contains(r#""method":"user.append""#)
                    && let Some(id) = field(&line, "id")
                    && let Some(answer) = injected(&id, &inject, &line)
                {
                    send(&answer);
                }
                // 搜索来源（施工 N-5）：查 `limit` 的说限额，查 `boom` 的出错，别的答一条。
                if line.contains(r#""method":"search.query""#)
                    && let Some(id) = field(&line, "id")
                {
                    send(&searched(&id, &field(&line, "query").unwrap_or_default()));
                }
                if line.contains(r#""method":"command.call""#)
                    && let Some(id) = field(&line, "id")
                    && let Some(answer) = commanded(
                        &id,
                        &field(&line, "command").unwrap_or_default(),
                        &field(&line, "argument").unwrap_or_default(),
                    )
                {
                    send(&answer);
                }
            }
        } else if let Some(path) = step.strip_prefix("pause:") {
            while !std::path::Path::new(path).exists() {
                std::thread::sleep(Duration::from_millis(10));
            }
        } else if step == "hang" {
            loop {
                std::thread::sleep(Duration::from_secs(60));
            }
        } else if let Some(code) = step.strip_prefix("exit:") {
            std::process::exit(code.parse().expect("退出码是数"));
        } else {
            panic!("不认识的一步：{step}");
        }
    }
}

/// 写一行到标准输出。写不出去（核心关了）的照样往下走。
fn send(line: &str) {
    let mut out = io::stdout().lock();
    if writeln!(out, "{line}").and_then(|()| out.flush()).is_err() {
        // 核心那头关了：后面的步子照做。
    }
}

/// 读一行；读到头、读不了的是空的。
fn read(input: &mut impl BufRead) -> String {
    let mut line = String::new();
    match input.read_line(&mut line) {
        Ok(_) => line.trim_end().to_string(),
        Err(_) => String::new(),
    }
}

/// 一行回应里 `"session":"…"` 的那个编号；没有的是没有。只用标准库，不解析整份 JSON。
fn session_in(line: &str) -> Option<String> {
    field(line, "session")
}

/// 一行里 `"<name>":"…"` 的那个字；没有的是没有。只用标准库，不解析整份 JSON。
fn field(line: &str, name: &str) -> Option<String> {
    let (_, rest) = line.split_once(&format!(r#""{name}":""#))?;
    let (value, _) = rest.split_once('"')?;
    Some(value.to_string())
}

/// 答编号是 `id`、工具是 `tool` 的那一次 `tool.call`（施工 O-2 上）；`silent` 不答，等核心超时、发 `tool.cancel`（施工 O-2 下）；
/// `skip`、`skip_error` 带 `end_turn`（施工 4-14）；`links`、`bad_detail`、`detail_error` 带 `detail`（施工 W-11 补）；`data`、
/// `data_error` 带 `data`（施工 X-33）。
fn served(id: &str, tool: &str, sent: &str) -> Option<String> {
    Some(match tool {
        // 一次发五张，多了（施工 X-19）。
        "too_many" => {
            let one =
                format!(r#"{{"blob":"{sent}","media_type":"image/png","width":2,"height":2}}"#);
            let five = vec![one; 5].join(",");
            format!(
                r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"blocks":[{{"type":"text","text":"Sent."}}],"sent":[{five}]}}}}"#
            )
        }
        // 发出去一张图（施工 X-19）：本机路径是扩展乱写的，核心不收。
        "sticker" => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"blocks":[{{"type":"text","text":"Sent m1."}}],"sent":[{{"blob":"{sent}","media_type":"image/png","width":2,"height":2,"alt":"A cat waving.","sticker":true,"path":"/etc/passwd"}}]}}}}"#
        ),
        "silent" => return None,
        "boom" => {
            format!(r#"{{"jsonrpc":"2.0","id":"{id}","error":{{"code":-32000,"message":"boom"}}}}"#)
        }
        "garbled" => format!(r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"nothing":1}}}}"#),
        "oops" => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"blocks":[{{"type":"text","text":"oops"}}],"error":true}}}}"#
        ),
        // 要结束这一轮的（施工 4-14）：通讯平台的 `skip_reply` 那样；出错的照样带上，核心不认。
        "skip" => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"blocks":[{{"type":"text","text":"skipped"}}],"end_turn":true}}}}"#
        ),
        "skip_error" => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"blocks":[{{"type":"text","text":"failed"}}],"error":true,"end_turn":true}}}}"#
        ),
        // 给头看的结构化结果（施工 W-11 补）：合形状的照转；形状不对的、出错时带的，核心丢掉。
        "links" => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"blocks":[{{"type":"text","text":"found links"}}],"detail":{{"kind":"links","links":[{{"title":"Miyu","url":"https://example.com/miyu","snippet":"A cat.","site":null}}]}}}}}}"#
        ),
        "bad_detail" => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"blocks":[{{"type":"text","text":"found bad"}}],"detail":{{"kind":"links","links":[{{"title":"Local","url":"file:///etc/passwd"}}]}}}}}}"#
        ),
        "detail_error" => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"blocks":[{{"type":"text","text":"failed"}}],"error":true,"detail":{{"kind":"links","links":[]}}}}}}"#
        ),
        // 给脚本的结构化结果（施工 X-33）：照收、交给嵌套调用；出错时带的，核心不交。
        "data" => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"blocks":[{{"type":"text","text":"found data"}}],"data":{{"items":[{{"n":1}}],"empty":[]}}}}}}"#
        ),
        "data_error" => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"blocks":[{{"type":"text","text":"failed"}}],"error":true,"data":{{"items":[]}}}}}}"#
        ),
        _ => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"blocks":[{{"type":"text","text":"served {tool}"}}]}}}}"#
        ),
    })
}

/// `method.call` 的 `params` 里的方法名：`"params":{` 后面的第一个 `"method":"…"`（施工 X-18：前面可以有 `from`）。
fn inner_method(line: &str) -> Option<String> {
    let (_, rest) = line.split_once(r#""params":{"#)?;
    field(rest, "method")
}

/// 答编号是 `id`、方法是 `method` 的那一次 `method.call`（施工 F-6 中）；`slow` 不答，等核心到时限；`give` 交回 `give` 那几段字
/// （施工 W-6 三补）。
fn answered(id: &str, method: &str, give: &str) -> Option<String> {
    Some(match method {
        "slow" => return None,
        "give" => {
            let given: Vec<String> = give.split(',').map(|one| format!("{one:?}")).collect();
            format!(
                r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"given":[{}]}}}}"#,
                given.join(",")
            )
        }
        "boom" => {
            format!(r#"{{"jsonrpc":"2.0","id":"{id}","error":{{"code":-32001,"message":"boom"}}}}"#)
        }
        _ => format!(r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"answered":"{method}"}}}}"#),
    })
}

/// 答编号是 `id`、查的是 `query` 的那一次 `search.query`（施工 N-5）。
fn searched(id: &str, query: &str) -> String {
    match query {
        "limit" => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","error":{{"code":-32000,"message":"quota used up","data":{{"reason":"limited"}}}}}}"#
        ),
        "boom" => {
            format!(r#"{{"jsonrpc":"2.0","id":"{id}","error":{{"code":-32000,"message":"boom"}}}}"#)
        }
        _ => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"hits":[{{"title":"found {query}","url":"https://ext.example/{query}"}}]}}}}"#
        ),
    }
}

/// 答编号是 `id`、命令是 `command`、跟的字是 `argument` 的那一次 `command.call`（施工 X-14 下）。
fn commanded(id: &str, command: &str, argument: &str) -> Option<String> {
    Some(match argument {
        "silent" => return None,
        "boom" => {
            format!(r#"{{"jsonrpc":"2.0","id":"{id}","error":{{"code":-32000,"message":"boom"}}}}"#)
        }
        "garbled" => format!(r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"nothing":1}}}}"#),
        _ => format!(
            r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"said":"{command} {argument}\nok"}}}}"#
        ),
    })
}

/// 答编号是 `id` 的那一次 `user.append`（施工 X-16），照 `how`。
fn injected(id: &str, how: &str, line: &str) -> Option<String> {
    let block = |text: &str| format!(r#"{{"kind":"meme","text":"{text}"}}"#);
    let blocks = match how {
        "hint" | "slow" => {
            if how == "slow" {
                std::thread::sleep(std::time::Duration::from_millis(600));
            }
            let venue = field(line, "venue").unwrap_or_default();
            let text = field(line, "text").unwrap_or_else(|| "-".to_string());
            block(&format!("hint {venue} {text}"))
        }
        "many" => vec![block("a"); 4].join(","),
        "empty" => String::new(),
        "boom" => {
            return Some(format!(
                r#"{{"jsonrpc":"2.0","id":"{id}","error":{{"code":-32000,"message":"boom"}}}}"#
            ));
        }
        "garbled" => {
            return Some(format!(
                r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"content":[{{"kind":"Bad Kind","text":"x"}}]}}}}"#
            ));
        }
        _ => return None,
    };
    Some(format!(
        r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"content":[{blocks}]}}}}"#
    ))
}

/// 推块 `widget` 的一段字（施工 X-15 中）：`<字> <块>`，跟着会话的后面接会话编号；`temp` 那一块只推字本身。
fn shown(widget: &str, session: Option<&str>, text: &str) -> String {
    let words = match (widget, session) {
        ("temp", _) => text.to_string(),
        (_, Some(session)) => format!("{text} {widget} {session}"),
        (_, None) => format!("{text} {widget}"),
    };
    let session = session.map_or_else(String::new, |session| format!(r#","session":"{session}""#));
    format!(
        r#"{{"jsonrpc":"2.0","id":"w-{widget}","method":"widget.set","params":{{"widget":"{widget}"{session},"content":[{{"type":"text","text":"{words}"}}]}}}}"#
    )
}

/// 记下一行。
fn keep(record: &mut Option<File>, line: &str) {
    if let Some(file) = record {
        writeln!(file, "{line}").expect("写得进");
    }
}
