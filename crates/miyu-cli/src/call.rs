//! `miyu call`（施工 M-3，设计 `39-脚本调工具.md` 第六节，`docs/blueprint/cli/main.md`「miyu call」）：在 Miyu 跑的命令里调
//! 这个会话的工具。连上环境变量 `MIYU_CALL_SOCK` 说的端点（Unix 套接字、Windows 命名管道），带上 `MIYU_CALL_TOKEN` 发一个请求，照答回来的印、退出。
//!
//! 不读清单、不连核心的协议、不读本机令牌：没有这两个变量的、连不上的一律退出码 2，不退回去经协议调（MC11：会话里跑在沙盒
//! 外的命令读得到本机令牌，改走协议就绕过了这个会话的权限）。
//!
//! 写法：`miyu call <工具> --<参数> <值>…`（值一律当字符串发，核心照工具的格式修；写几次的是数组），`--json '<对象>'`、
//! `--json -`（从标准输入读），具名的盖过 `--json` 里同名的；`miyu call --schema <工具>…`、`miyu call <工具> --schema`。
//! 给她看的几句和核心那头的放在一起（`resources/software/basesystem/shell/call/`），编进程序。

use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::ExitCode;

use serde_json::{Map, Value};

use crate::help::{Page, page};
use crate::language::Language;

/// 用法不对，没执行。
const USAGE: u8 = 2;

/// 所属的命令已经结束、被打断。
#[cfg_attr(
    not(any(unix, windows)),
    expect(dead_code, reason = "别的系统没有通道")
)]
const ENDED: u8 = 3;

/// 协议版本，和核心那一头一样。
const VERSION: u32 = 1;

/// 不在 Miyu 跑的命令里。
const OUTSIDE: &str = include_str!("../../../resources/software/basesystem/shell/call/outside.txt");

/// 所属的命令已经结束。
#[cfg_attr(
    not(any(unix, windows)),
    expect(dead_code, reason = "别的系统没有通道")
)]
const ENDED_TEXT: &str =
    include_str!("../../../resources/software/basesystem/shell/call/ended.txt");

/// 怎么写。
const HOW: &str = include_str!("../../../resources/software/basesystem/shell/call/usage.txt");

/// `--json` 给的不是一个对象。
const BAD_JSON: &str =
    include_str!("../../../resources/software/basesystem/shell/call/bad-json.txt");

/// `miyu call` 后面的参数 `args`；`-h`、`--help` 印 `language` 的帮助页。
pub fn call(args: &[OsString], language: Language) -> ExitCode {
    let args: Vec<String> = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    if matches!(args.first().map(String::as_str), Some("-h" | "--help")) {
        print!("{}", page(language, Page::Call));
        return ExitCode::SUCCESS;
    }
    let requests = match parse(&args) {
        Ok(requests) => requests,
        Err(why) => return refuse(USAGE, &why),
    };
    // `--schema` 后面跟几个名字的一件一件问、挨着印；有一件不成的照印，退出码取最后不成的那件的。
    let mut code = 0;
    for request in &requests {
        let got = match send(request) {
            Ok(reply) => {
                let text = |key: &str| {
                    reply
                        .get(key)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                };
                print_out(&text("stdout"), &text("stderr"));
                reply
                    .get("code")
                    .and_then(Value::as_u64)
                    .and_then(|code| u8::try_from(code).ok())
                    .unwrap_or(1)
            }
            Err((got, why)) => {
                print_out("", &why);
                got
            }
        };
        if got != 0 {
            code = got;
        }
    }
    ExitCode::from(code)
}

/// 印出来：标准输出、标准错误各自原样。写不进去的（管道那头关了）不管：结果已经交了。
#[expect(
    clippy::let_underscore_must_use,
    reason = "管道那头关了：脚本不读了，印不出来也没人要"
)]
fn print_out(stdout: &str, stderr: &str) {
    let _ = std::io::stdout().write_all(stdout.as_bytes());
    let _ = std::io::stderr().write_all(stderr.as_bytes());
}

/// 印一句、退出。
fn refuse(code: u8, why: &str) -> ExitCode {
    print_out("", why);
    ExitCode::from(code)
}

/// 照参数造请求：调一件是一个，`--schema` 几件是几个。
fn parse(args: &[String]) -> Result<Vec<Value>, String> {
    let schema = |tool: &str| serde_json::json!({"v": VERSION, "op": "schema", "tool": tool});
    let Some(first) = args.first() else {
        return Err(HOW.to_string());
    };
    if first == "--schema" {
        return match args.len() {
            1 => Err(HOW.to_string()),
            _ => Ok(args[1..].iter().map(|tool| schema(tool)).collect()),
        };
    }
    if first.starts_with('-') {
        return Err(HOW.to_string());
    }
    if args.len() == 2 && args[1] == "--schema" {
        return Ok(vec![schema(first)]);
    }
    let mut named = Map::new();
    let mut json: Option<String> = None;
    let mut rest = args[1..].iter();
    while let Some(arg) = rest.next() {
        let Some(flag) = arg.strip_prefix("--") else {
            return Err(HOW.to_string());
        };
        let (key, value) = match flag.split_once('=') {
            Some((key, value)) => (key.to_string(), value.to_string()),
            None => match rest.next() {
                Some(value) => (flag.to_string(), value.clone()),
                None => return Err(HOW.to_string()),
            },
        };
        if key == "json" {
            json = Some(match value.as_str() {
                "-" => {
                    let mut text = String::new();
                    std::io::stdin()
                        .read_to_string(&mut text)
                        .map_err(|_| BAD_JSON.to_string())?;
                    text
                }
                _ => value,
            });
            continue;
        }
        // 写几次的是数组；值一律是字符串，核心照工具的格式修。
        match named.get_mut(&key) {
            Some(Value::Array(items)) => items.push(Value::String(value)),
            Some(existing) => {
                let first = existing.take();
                *existing = Value::Array(vec![first, Value::String(value)]);
            }
            None => {
                named.insert(key, Value::String(value));
            }
        }
    }
    let mut object = match json {
        None => Map::new(),
        Some(text) => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(object)) => object,
            _ => return Err(BAD_JSON.to_string()),
        },
    };
    // 具名的盖过 `--json` 里同名的。
    object.extend(named);
    let args = Value::Object(object).to_string();
    Ok(vec![
        serde_json::json!({"v": VERSION, "op": "call", "tool": first, "args": args}),
    ])
}

/// 连上环境变量里的端点，带上令牌发请求、收回答：4 字节大端长度加 JSON，一次连接一个。端点不在了、没人听的是所属的命令
/// 结束了（退出码 3）；没有那两个变量、别的连不上的不在 Miyu 跑的命令里（退出码 2）。
#[cfg(any(unix, windows))]
fn send(request: &Value) -> Result<Map<String, Value>, (u8, String)> {
    let outside = || (USAGE, OUTSIDE.to_string());
    let ended = || (ENDED, ENDED_TEXT.to_string());
    let endpoint = std::env::var_os("MIYU_CALL_SOCK").ok_or_else(outside)?;
    let token = std::env::var("MIYU_CALL_TOKEN").map_err(|_| outside())?;
    let mut stream = connect(&endpoint).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => ended(),
        _ => outside(),
    })?;
    let mut request = request.clone();
    request["token"] = Value::String(token);
    let body = request.to_string();
    let len = u32::try_from(body.len()).map_err(|_| (USAGE, BAD_JSON.to_string()))?;
    stream.write_all(&len.to_be_bytes()).map_err(|_| ended())?;
    stream.write_all(body.as_bytes()).map_err(|_| ended())?;
    let mut head = [0u8; 4];
    stream.read_exact(&mut head).map_err(|_| ended())?;
    let mut reply = vec![0u8; u32::from_be_bytes(head) as usize];
    stream.read_exact(&mut reply).map_err(|_| ended())?;
    match serde_json::from_slice::<Value>(&reply) {
        Ok(Value::Object(reply)) => Ok(reply),
        _ => Err(ended()),
    }
}

/// Unix：连套接字。
#[cfg(unix)]
fn connect(endpoint: &std::ffi::OsStr) -> std::io::Result<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect(endpoint)
}

/// Windows：命名管道当文件开，只要连上、读写的权限（`miyu_pipe::CONNECT_ACCESS`：沙盒里的命令只给了这些）。几个实例都忙着的
/// （`ERROR_PIPE_BUSY`），等一会儿再试，最多两秒。
#[cfg(windows)]
fn connect(endpoint: &std::ffi::OsStr) -> std::io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const PIPE_BUSY: i32 = 231;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match std::fs::OpenOptions::new()
            .access_mode(miyu_pipe::CONNECT_ACCESS)
            .open(endpoint)
        {
            Err(error)
                if error.raw_os_error() == Some(PIPE_BUSY)
                    && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            opened => return opened,
        }
    }
}

/// 别的系统没有通道：照不在 Miyu 跑的命令里说。
#[cfg(not(any(unix, windows)))]
fn send(_request: &Value) -> Result<Map<String, Value>, (u8, String)> {
    Err((USAGE, OUTSIDE.to_string()))
}

#[cfg(test)]
mod tests;
