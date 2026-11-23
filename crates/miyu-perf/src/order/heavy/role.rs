//! 重场景里的扩展、平台桥（施工 LW-30）：量尺自己扮。核心拉起的是拷进包里的量尺，参数是 `[ARG, <剧本>, <控制目录>, <记录>]`。
//!
//! 一个线程读标准输入：核心发来的请求当场答（[`answer`]），自己发的请求的回应交给等着的那一步，会话的事件攒着给
//! `turn` 那一步；读到头（核心请它退出）整个进程退出。主线程照剧本一步步做（剧本的写法见下），每一步的结果追加进记录文件，一行
//! 一条 JSON，量尺照它等、照它查错（`heavy/steps.rs` 的 `finished`）。
//!
//! 剧本一步是一个对象，认这几种：
//!
//! - `{"call": 方法, "params": {...}}`：发请求，等回应。参数里整个是 `"{session}"` 的字换成最近一条带会话编号的回应里的编号，
//!   `"{s:名字}"` 换成 `save` 记下的，`"{seq}"` 换成最近一次 `session.send` 回应里这一句的序号（是数，不是字）；
//! - `{"save": 名字}`：把最近一条带会话编号的回应里的编号记成这个名字；
//! - `{"pause": 名字}`：等控制目录里有了这个文件；
//! - `{"once": 名字}`：控制目录里已经有这个文件的（重启以后又起来的），后面的步子都不做，只答请求；没有的建上、接着做；
//! - `{"turn": 会话}`：等这个会话推来 `turn.ended`（要先订阅它的事件），最多 [`PATIENCE`]。
//!
//! 剧本做完记一行 `done`。

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// 量尺照这个参数起来就当扩展（[`serve`]）。
pub const ARG: &str = "heavy-extension";

/// 等一个回应、一轮结束最多多久：核心在模拟器里。
const PATIENCE: Duration = Duration::from_secs(600);

/// 等控制文件时多久看一次。
const POLL: Duration = Duration::from_millis(50);

/// 写标准输出、等回应的几样，读的线程和照剧本做的主线程共用。
struct Link {
    out: Mutex<std::io::Stdout>,
    waiting: Mutex<HashMap<String, Sender<Value>>>,
}

impl Link {
    fn send(&self, message: &Value) {
        let mut out = self.out.lock().unwrap_or_else(PoisonError::into_inner);
        // 写不出去说明核心走了：读的那一头随后读到头、整个进程退出，这里不用另外处理。
        if writeln!(out, "{message}")
            .and_then(|()| out.flush())
            .is_err()
        {
            std::process::exit(0);
        }
    }
}

/// 当扩展：照 `words`（`[ARG, 剧本, 控制目录, 记录]`）握手、做剧本、一直答到标准输入读到头。
pub fn serve(words: &[String]) -> std::process::ExitCode {
    let [_, plan, ctl, record] = words else {
        eprintln!("用法：miyu-perf {ARG} <剧本.json> <控制目录> <记录>");
        return std::process::ExitCode::FAILURE;
    };
    let steps: Vec<Value> = match std::fs::read_to_string(plan)
        .map_err(|e| e.to_string())
        .and_then(|text| serde_json::from_str(&text).map_err(|e| e.to_string()))
    {
        Ok(steps) => steps,
        Err(e) => {
            eprintln!("剧本 {plan} 读不了：{e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let mut log = match OpenOptions::new().create(true).append(true).open(record) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("记录 {record} 开不了：{e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let link = Arc::new(Link {
        out: Mutex::new(std::io::stdout()),
        waiting: Mutex::new(HashMap::new()),
    });
    let (events, turns) = mpsc::channel();
    let reading = Arc::clone(&link);
    std::thread::spawn(move || read(&reading, &events));
    let mut keep = |kind: &str, body: Value| {
        // 记不下的只是量尺那一头等不到、报出来；扩展照样答核心。
        if writeln!(log, "{}", json!({"kind": kind, "body": body})).is_err() {
            eprintln!("记录写不进 {record}");
        }
    };
    let mut state = State::default();
    let hello = call(
        &link,
        &mut state,
        "hello",
        json!({"protocol": [1, 1], "head": {"kind": "perf-heavy", "version": "0"}}),
    );
    keep("hello", json!({"ok": hello.get("result").is_some()}));
    for step in &steps {
        if let Some(name) = step["pause"].as_str() {
            while !Path::new(ctl).join(name).exists() {
                std::thread::sleep(POLL);
            }
        } else if let Some(name) = step["once"].as_str() {
            let mark = Path::new(ctl).join(name);
            if mark.exists() {
                break;
            }
            if std::fs::write(&mark, b"").is_err() {
                keep(
                    "call",
                    json!({"method": "once", "error": format!("建不了 {}", mark.display())}),
                );
            }
        } else if let Some(name) = step["save"].as_str() {
            state.saved.insert(name.to_string(), state.session.clone());
        } else if let Some(session) = step.get("turn") {
            let session = state.fill(session);
            let ended = wait_turn(&turns, &session);
            keep("turn", json!({"session": session, "ended": ended}));
        } else if let Some(method) = step["call"].as_str() {
            let params = state.fill(&step["params"]);
            let reply = call(&link, &mut state, method, params);
            keep(
                "call",
                json!({"method": method, "error": reply.get("error")}),
            );
        }
    }
    keep("done", json!({}));
    loop {
        std::thread::park();
    }
}

/// 照剧本做时记着的：最近的会话编号、`save` 记下的、最近一句的序号。
#[derive(Default)]
struct State {
    session: Value,
    saved: HashMap<String, Value>,
    seq: Value,
    next: u64,
}

impl State {
    /// 换掉参数里的占位（见模块开头）。
    fn fill(&self, value: &Value) -> Value {
        match value {
            Value::String(text) if text == "{session}" => self.session.clone(),
            Value::String(text) if text == "{seq}" => self.seq.clone(),
            Value::String(text) if text.starts_with("{s:") && text.ends_with('}') => self
                .saved
                .get(&text[3..text.len() - 1])
                .cloned()
                .unwrap_or(Value::Null),
            Value::Array(items) => Value::Array(items.iter().map(|item| self.fill(item)).collect()),
            Value::Object(map) => {
                Value::Object(map.iter().map(|(k, v)| (k.clone(), self.fill(v))).collect())
            }
            other => other.clone(),
        }
    }
}

/// 发一条请求，等它的回应；回应里带会话编号、句子序号的记下来。等不到的交回一条写着超时的错误。
fn call(link: &Link, state: &mut State, method: &str, params: Value) -> Value {
    state.next += 1;
    let id = format!("heavy-{}", state.next);
    let (sender, receiver) = mpsc::channel();
    link.waiting
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(id.clone(), sender);
    link.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
    let reply = receiver
        .recv_timeout(PATIENCE)
        .unwrap_or_else(|_| json!({"error": {"message": format!("{method} timed out")}}));
    if let Some(session) = reply["result"].get("session").filter(|s| s.is_string()) {
        state.session = session.clone();
    }
    if method == "session.send"
        && let Some(seq) = reply["result"]["events"].get(0)
    {
        state.seq = seq.clone();
    }
    reply
}

/// 等 `session` 推来 `turn.ended`：等到了是真。
fn wait_turn(turns: &Receiver<Value>, session: &Value) -> bool {
    let began = Instant::now();
    while let Some(left) = PATIENCE.checked_sub(began.elapsed()) {
        let Ok(push) = turns.recv_timeout(left) else {
            return false;
        };
        if push["params"]["session"] == *session && push["params"]["event"]["kind"] == "turn.ended"
        {
            return true;
        }
    }
    false
}

/// 读的线程：回应交给等着的，请求当场在新线程里答（几个调用会同时到），会话的事件交给 `events`。读到头整个进程退出。
fn read(link: &Arc<Link>, events: &Sender<Value>) {
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else {
            break;
        };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let id = message["id"].as_str().map(str::to_string);
        if message.get("method").is_none() {
            let sender = id.and_then(|id| {
                link.waiting
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(&id)
            });
            if let Some(sender) = sender
                && sender.send(message).is_err()
            {
                eprintln!("等回应的一步已经不等了");
            }
        } else if message.get("id").is_some() {
            let link = Arc::clone(link);
            std::thread::spawn(move || {
                let reply = json!({"jsonrpc": "2.0", "id": message["id"], "result": answer(&message, &link)});
                link.send(&reply);
            });
        } else if message["method"] == "event" && events.send(message).is_err() {
            break;
        }
    }
    std::process::exit(0);
}

/// 答核心发来的请求：工具调用回一段字、一张表、给脚本的 `data`；回合开始的问话交一块；守卫放行；斜杠命令回两行；后台页
/// 方法回一份；搜索交五条；有人看界面块时推一块（`widget.set`）。别的回空的结果。
fn answer(message: &Value, link: &Link) -> Value {
    let params = &message["params"];
    match message["method"].as_str().unwrap_or_default() {
        "tool.call" => json!({
            "blocks": [{"type": "text", "text": format!("{} found 3 entries for {}.", params["tool"], params["args"])}],
            "detail": {"kind": "table", "columns": ["id", "title"], "rows": [["1", "alpha"], ["2", "beta"], ["3", "gamma"]]},
            "data": {"items": [{"id": 1}, {"id": 2}]}}),
        "user.append" => json!({"content": [{"kind": "perf-hint",
            "text": format!("<perf-hint venue=\"{}\">nothing new</perf-hint>", params["venue"].as_str().unwrap_or_default())}]}),
        "tool.guard" => json!({"verdict": "allow"}),
        "command.call" => {
            json!({"said": format!("{} {}\nok", params["command"].as_str().unwrap_or_default(),
            params["argument"].as_str().unwrap_or_default())})
        }
        "method.call" => json!({"answered": params["method"], "items": [1, 2, 3]}),
        "search.query" => {
            let query = params["query"].as_str().unwrap_or_default();
            let hits: Vec<Value> = (0..5)
                .map(|k| json!({"title": format!("Result {k} for {query}"), "url": format!("https://example.com/{k}"),
                    "snippet": "An excerpt.", "date": "2026-11-01"}))
                .collect();
            json!({"hits": hits})
        }
        "widget.watch" => {
            let mut set = json!({"widget": params["widget"], "content": [{"type": "heading", "text": "Heavy"},
                {"type": "pairs", "items": [{"label": "jobs", "value": "3"}]},
                {"type": "list", "items": [{"title": "build #1", "right": "3m", "state": "doing"}]}]});
            if let Some(session) = params.get("session").filter(|s| s.is_string()) {
                set["session"] = session.clone();
            }
            link.send(&json!({"jsonrpc": "2.0", "method": "widget.set", "params": set}));
            json!({})
        }
        _ => json!({}),
    }
}

#[cfg(test)]
mod tests;
