//! 登录的回调（施工 X-42 上）：只听字面的 `127.0.0.1`（不解析 `localhost`、不听 `0.0.0.0`），端口照记着的、没有的随机；收到
//! [`CALLBACK_PATH`] 的那一个请求交给调的一方判，别的路径（浏览器顺手要的 `/favicon.ico`）回 `404`、接着等。回给浏览器的那一页
//! 由调的一方给（照人的语言）。

use std::io;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use super::{CALLBACK_PATH, Callback, callback};

/// 一行请求最长多少字节：回调的网址带授权码、`state`、`iss`，几 KiB 足够。
const LINE_LIMIT: usize = 16 * 1024;

/// 听着的回调。
pub struct Listener {
    inner: TcpListener,
    port: u16,
}

/// 听 `127.0.0.1` 的 `port`（没写的随机一个）。
///
/// # Errors
///
/// 端口被占了、开不了。
pub async fn listen(port: Option<u16>) -> io::Result<Listener> {
    let inner = TcpListener::bind(("127.0.0.1", port.unwrap_or(0))).await?;
    let port = inner.local_addr()?.port();
    Ok(Listener { inner, port })
}

impl Listener {
    /// 听着的端口。
    pub fn port(&self) -> u16 {
        self.port
    }

    /// 回调的地址：`http://127.0.0.1:<端口>/callback`。
    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}{CALLBACK_PATH}", self.port)
    }

    /// 等到回调：交回读出来的和回浏览器那一页的一头。不限时：调的一方自己限。
    ///
    /// # Errors
    ///
    /// 听着的端口出了错。
    pub async fn next(&self) -> io::Result<(Callback, Reply)> {
        loop {
            let (stream, _) = self.inner.accept().await?;
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            let read = (&mut reader)
                .take(LINE_LIMIT as u64)
                .read_line(&mut line)
                .await;
            let target = match read {
                Ok(_) => line.split_whitespace().nth(1).map(str::to_string),
                Err(_) => None,
            };
            let mut stream = reader.into_inner();
            match target.as_deref().and_then(callback) {
                Some(found) => return Ok((found, Reply(stream))),
                None => drop(write(&mut stream, 404, "").await),
            }
        }
    }
}

/// 回浏览器的那一头。
pub struct Reply(TcpStream);

impl Reply {
    /// 回一页 HTML：`ok` 是 `200`，不然 `400`。写不出去的不管：人关了浏览器也不影响登录。
    pub async fn send(mut self, ok: bool, page: &str) {
        drop(write(&mut self.0, if ok { 200 } else { 400 }, page).await);
    }
}

/// 写一个回应。
async fn write(stream: &mut TcpStream, status: u16, page: &str) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        page.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(page.as_bytes()).await?;
    stream.shutdown().await
}
