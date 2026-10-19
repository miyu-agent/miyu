//! `miyu call`（设计 39 第六节，原型 M-0）：在 Miyu 起的命令里调这个会话的工具。经编号 3 上的入口发一对私有的流过去，
//! 在上面发请求、收结局，照结局印、退出。不在 Miyu 起的命令里的一律退出码 2，不改走核心的协议（设计 39 MC11）。

use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::ExitCode;

use serde_json::{Map, Value};

/// 用法不对。
const USAGE: u8 = 2;

/// 所属的命令已经结束、被打断。
const ENDED: u8 = 3;

/// 协议版本，和核心那一头一样。
const VERSION: u32 = 1;

/// `miyu call` 后面的参数 `args`。
pub fn call(args: &[OsString]) -> ExitCode {
    let args: Vec<String> = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    // `--schema` 后面跟好几个名字的：一件一件问，挨着印；有一件不成的，退出码取最后不成的那件的。
    if args.first().is_some_and(|first| first == "--schema") && args.len() > 2 {
        let mut code = 0u8;
        for tool in &args[1..] {
            let request = serde_json::json!({"v": VERSION, "op": "schema", "tool": tool});
            match send(&request) {
                Ok(reply) => {
                    let text = |key: &str| reply.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
                    let _ = std::io::stdout().write_all(text("stdout").as_bytes());
                    let _ = std::io::stderr().write_all(text("stderr").as_bytes());
                    let got = reply.get("code").and_then(Value::as_u64).unwrap_or(1);
                    if got != 0 {
                        code = u8::try_from(got).unwrap_or(1);
                    }
                }
                Err((got, why)) => {
                    eprintln!("{why}");
                    code = got;
                }
            }
        }
        return ExitCode::from(code);
    }
    let request = match parse(&args) {
        Ok(request) => request,
        Err(why) => return refuse(USAGE, &why),
    };
    match send(&request) {
        Ok(reply) => {
            let text = |key: &str| reply.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
            let _ = std::io::stdout().write_all(text("stdout").as_bytes());
            let _ = std::io::stderr().write_all(text("stderr").as_bytes());
            let code = reply.get("code").and_then(Value::as_u64).unwrap_or(1);
            ExitCode::from(u8::try_from(code).unwrap_or(1))
        }
        Err((code, why)) => refuse(code, &why),
    }
}

/// 印一句、退出。
fn refuse(code: u8, why: &str) -> ExitCode {
    eprintln!("{why}");
    ExitCode::from(code)
}

/// 照参数造请求。
fn parse(args: &[String]) -> Result<Value, String> {
    const HOW: &str = "Usage: miyu call <tool> --<param> <value>... | miyu call <tool> --json '<object>' | miyu call --schema <tool>...";
    let mut rest = args.iter();
    let Some(first) = rest.next() else {
        return Err(HOW.into());
    };
    if first == "--schema" {
        let Some(tool) = rest.next() else {
            return Err(HOW.into());
        };
        return Ok(serde_json::json!({"v": VERSION, "op": "schema", "tool": tool}));
    }
    if first.starts_with('-') {
        return Err(HOW.into());
    }
    let tool = first.clone();
    let mut named = Map::new();
    let mut json: Option<String> = None;
    while let Some(arg) = rest.next() {
        let Some(flag) = arg.strip_prefix("--") else {
            return Err(format!("Unexpected {arg}. {HOW}"));
        };
        let (key, value) = match flag.split_once('=') {
            Some((key, value)) => (key.to_string(), value.to_string()),
            None => match rest.next() {
                Some(value) => (flag.to_string(), value.clone()),
                None => return Err(format!("--{flag} needs a value.")),
            },
        };
        if key == "json" {
            json = Some(if value == "-" {
                let mut text = String::new();
                std::io::stdin()
                    .read_to_string(&mut text)
                    .map_err(|error| format!("Could not read stdin: {error}"))?;
                text
            } else {
                value
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
            _ => return Err("--json must be a JSON object.".into()),
        },
    };
    // 具名的盖过 `--json` 里同名的；单个字符串里的数字、布尔由核心照工具的格式修。
    object.extend(named);
    let args = Value::Object(object).to_string();
    Ok(serde_json::json!({"v": VERSION, "op": "call", "tool": tool, "args": args}))
}

/// 经入口发一对私有的流过去，发请求、收回答。
#[cfg(unix)]
fn send(request: &Value) -> Result<Map<String, Value>, (u8, String)> {
    use std::os::fd::{AsFd, BorrowedFd};
    use std::os::unix::net::UnixStream;

    use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};

    let outside = || (USAGE, "miyu call only works inside a command Miyu runs.".to_string());
    let ended = || (ENDED, "The command this call belongs to has ended.".to_string());
    let fd: i32 = std::env::var("MIYU_CALL_FD")
        .ok()
        .and_then(|fd| fd.parse().ok())
        .ok_or_else(outside)?;
    let (mut ours, theirs) = UnixStream::pair().map_err(|_| ended())?;
    #[allow(unsafe_code)]
    // SAFETY: 只在这一次 sendmsg 里借用编号 `fd`，不关它、不存它；编号不是打开着的描述符时系统调用报 EBADF，下面照「不在 Miyu
    // 起的命令里」答。
    let entry = unsafe { BorrowedFd::borrow_raw(fd) };
    let fds = [theirs.as_fd()];
    let mut space = [std::mem::MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    ancillary.push(SendAncillaryMessage::ScmRights(&fds));
    match sendmsg(entry, &[std::io::IoSlice::new(b"c")], &mut ancillary, SendFlags::empty()) {
        Ok(_) => {}
        Err(rustix::io::Errno::BADF | rustix::io::Errno::NOTSOCK) => return Err(outside()),
        Err(_) => return Err(ended()),
    }
    drop(theirs);
    let body = request.to_string();
    let len = u32::try_from(body.len()).map_err(|_| (USAGE, "Arguments are too long.".to_string()))?;
    ours.write_all(&len.to_be_bytes()).map_err(|_| ended())?;
    ours.write_all(body.as_bytes()).map_err(|_| ended())?;
    let mut head = [0u8; 4];
    ours.read_exact(&mut head).map_err(|_| ended())?;
    let mut reply = vec![0u8; u32::from_be_bytes(head) as usize];
    ours.read_exact(&mut reply).map_err(|_| ended())?;
    match serde_json::from_slice::<Value>(&reply) {
        Ok(Value::Object(reply)) => Ok(reply),
        _ => Err(ended()),
    }
}

/// 别的平台还没有通道（设计 39 第五节第 7 条）。
#[cfg(not(unix))]
fn send(_request: &Value) -> Result<Map<String, Value>, (u8, String)> {
    Err((USAGE, "miyu call is not supported on this platform yet.".to_string()))
}
