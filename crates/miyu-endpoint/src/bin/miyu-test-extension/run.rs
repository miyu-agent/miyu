//! 跑命令的那一件（施工 XG-1 下，设计 `63-扩展工具的效果和声明.md` 第六节第 2 条）：`serve` 答 `ext_run` 时照 `tool.call` 的
//! `channel` 起自己当 `miyu call`（[`client`]），把 `sock`、`token` 设成 `MIYU_CALL_SOCK`、`MIYU_CALL_TOKEN`，调参数 `call` 写的
//! 那件工具；`hold:` 给了路径的，等那个路径有了再答（测试趁这一会儿拿着令牌从别处连）。
//!
//! 测试会话 e0 的复核另加三样：`hold:late:<路径>|<文件>` 等信号再调、结局写进文件；`call` 写成 `@self:<工具>` 的扩展进程自己连；
//! `<工具>?path=<路径>` 带一个 `path` 参数（照会话的权限链判路径）。

use std::io::{Read, Write};

use super::{field, quoted};

/// 答编号是 `id` 的那一次 `ext_run`：`line` 是核心发来的那一行。答的字是子进程印的回答（没有通道的写 `no channel`），后面跟
/// ` code=<退出码>`。`hold` 是 `hold:` 那一步给的：写成 `late:<路径>|<文件>` 的先等 `<路径>` 有了再调、调的结局写进 `<文件>`、调完就答
/// （测试会话 e0 的复核：调用超时、被掐以后通道还认不认），别的是答之前等它有了的路径。参数 `call` 是 `@self:<工具>` 的扩展进程
/// 自己连上去调。
pub(crate) fn run(id: &str, line: &str, hold: &str) -> String {
    let late = hold
        .strip_prefix("late:")
        .and_then(|paths| paths.split_once('|'));
    let hold = if late.is_some() { "" } else { hold };
    if let Some((flag, _)) = late {
        wait_for(flag);
    }
    let channel = line
        .split_once(r#""channel":"#)
        .map(|(_, rest)| rest.to_string());
    let said = match channel {
        Some(channel) => {
            let sock = field(&channel, "sock")
                .unwrap_or_default()
                .replace("\\\\", "\\");
            let token = field(&channel, "token").unwrap_or_default();
            let tool = field(line, "call").unwrap_or_default();
            match tool.strip_prefix("@self:") {
                Some(tool) => {
                    let reply = connect(&sock)
                        .and_then(|mut stream| ask(&mut stream, &request(tool, &token)));
                    match reply {
                        Some(reply) => format!("{reply} code={}", code_of(&reply)),
                        None => "code=3".to_string(),
                    }
                }
                None => {
                    let exe = std::env::current_exe().expect("找得到自己");
                    let output = std::process::Command::new(exe)
                        .args(["client", &tool])
                        .env("MIYU_CALL_SOCK", sock)
                        .env("MIYU_CALL_TOKEN", token)
                        .output()
                        .expect("起得来");
                    format!(
                        "{} code={}",
                        String::from_utf8_lossy(&output.stdout).trim(),
                        output.status.code().unwrap_or(-1)
                    )
                }
            }
        }
        None => "no channel".to_string(),
    };
    if let Some((_, out)) = late {
        std::fs::write(out, &said).expect("写得进");
    }
    if !hold.is_empty() {
        wait_for(hold);
    }
    format!(
        r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"blocks":[{{"type":"text","text":"{}"}}]}}}}"#,
        quoted(&said)
    )
}

/// 等到 `path` 有了。
fn wait_for(path: &str) {
    while !std::path::Path::new(path).exists() {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// 调工具 `tool` 的一帧，令牌是 `token`：写成 `<工具>?path=<路径>` 的参数是 `{"path": <路径>}`（测试会话 e0 的复核：照会话的
/// 权限链判路径），别的参数是 `{}`。
fn request(tool: &str, token: &str) -> String {
    let (tool, args) = match tool.split_once("?path=") {
        Some((tool, path)) => (tool, format!(r#"{{"path":"{}"}}"#, quoted(path))),
        None => (tool, "{}".to_string()),
    };
    format!(
        r#"{{"v":1,"op":"call","tool":"{}","args":"{}","token":"{}"}}"#,
        quoted(tool),
        quoted(&args),
        quoted(token)
    )
}

/// 回答里的 `code`；读不出的当 3。
fn code_of(reply: &str) -> i32 {
    reply
        .split_once(r#""code":"#)
        .and_then(|(_, rest)| {
            rest.split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|code| code.parse().ok())
        })
        .unwrap_or(3)
}

/// 当 `miyu call`：照 `MIYU_CALL_SOCK`、`MIYU_CALL_TOKEN` 连上去，调工具 `tool`（参数 `{}`），回答的一行 JSON 原样印到标准输出，
/// 照回答的 `code` 退出。连不上、读到结尾的退出码 3（同 `miyu call`：所属的那一次已经完了）。
pub(crate) fn client(tool: &str) -> ! {
    let sock = std::env::var("MIYU_CALL_SOCK").unwrap_or_default();
    let token = std::env::var("MIYU_CALL_TOKEN").unwrap_or_default();
    let Some(reply) =
        connect(&sock).and_then(|mut stream| ask(&mut stream, &request(tool, &token)))
    else {
        std::process::exit(3);
    };
    println!("{reply}");
    std::process::exit(code_of(&reply));
}

/// 发一帧（4 字节大端长度加 JSON），读一帧回来。
fn ask(stream: &mut (impl Read + Write), request: &str) -> Option<String> {
    let len = u32::try_from(request.len()).ok()?;
    stream.write_all(&len.to_be_bytes()).ok()?;
    stream.write_all(request.as_bytes()).ok()?;
    let mut len = [0u8; 4];
    stream.read_exact(&mut len).ok()?;
    let mut body = vec![0u8; usize::try_from(u32::from_be_bytes(len)).ok()?];
    stream.read_exact(&mut body).ok()?;
    String::from_utf8(body).ok()
}

/// 连端点：Unix 上是套接字。
#[cfg(unix)]
fn connect(sock: &str) -> Option<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect(sock).ok()
}

/// 连端点：Windows 上是命名管道，当文件开。同 `miyu call`：几个实例都忙着的（`ERROR_PIPE_BUSY`）等一会儿再试，最多两秒（核心
/// 一个连上来以后才建下一个实例，和测试同时来连会撞上没有空实例的那一小会儿）。
#[cfg(windows)]
fn connect(sock: &str) -> Option<std::fs::File> {
    const PIPE_BUSY: i32 = 231;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(sock)
        {
            Err(error)
                if error.raw_os_error() == Some(PIPE_BUSY)
                    && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            opened => return opened.ok(),
        }
    }
}

/// 别的系统没有通道。
#[cfg(not(any(unix, windows)))]
fn connect(_sock: &str) -> Option<std::fs::File> {
    None
}
