//! 带一次性码开浏览器的那两样（`web-ui.md`「怎么走」第二条第 3、4 款）：照终端的样子连核心（出示本机令牌）、一问一答，
//! 要 `account.setup_code`；用系统的办法把网址交给浏览器。网址怎么拼、什么时候带码、印什么，各家照自己的命令定。

use std::process::Command;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};

use miyu_ipc::Connection;
use miyu_store::root::DataRoot;

use crate::CoreCommand;

/// 怎么把网址交给浏览器：交出去了的是真。
pub trait Browser {
    /// 打开 `url`。
    fn open(&self, url: &str) -> bool;
}

/// 系统的办法：Linux 是 `xdg-open`，macOS 是 `open`，Windows 是 `cmd /C start "" <网址>`。
pub struct SystemBrowser;

impl Browser for SystemBrowser {
    fn open(&self, url: &str) -> bool {
        let mut command = if cfg!(target_os = "macos") {
            Command::new("open")
        } else if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.args(["/C", "start", ""]);
            command
        } else {
            Command::new("xdg-open")
        };
        command
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        command.status().is_ok_and(|status| status.success())
    }
}

/// 核心拒了、连不上、没回（施工 W-8 再补，接入通讯平台要的）：`reason` 是核心的 `data.reason`（`bad_password`、`bad_login`、
/// `login_throttled`、`bad_ticket`……），核心之外的几种是固定的词：`unreachable`（连不上、拉不起来）、`no_answer`（30 秒没回）、
/// `disconnected`（核心断开）、`io`（写不出去、读不了）。`message` 是原话；`retry_after_ms` 是登录被挡时还要等多久。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// 为什么：照它写给人看的字，不照 `message` 猜。
    pub reason: String,
    /// 原话。
    pub message: String,
    /// `login_throttled` 的 `data.retry_after_ms`。
    pub retry_after_ms: Option<u64>,
}

impl Refused {
    /// 核心之外的一种：`reason` 是固定的词，`message` 是原话。
    fn local(reason: &str, message: impl std::fmt::Display) -> Refused {
        Refused {
            reason: reason.to_string(),
            message: message.to_string(),
            retry_after_ms: None,
        }
    }

    /// 核心回的拒绝 `error`。
    pub(crate) fn of(error: &Value) -> Refused {
        Refused {
            reason: error["data"]["reason"]
                .as_str()
                .unwrap_or("refused")
                .to_string(),
            message: error["message"].as_str().unwrap_or("refused").to_string(),
            retry_after_ms: error["data"]["retry_after_ms"].as_u64(),
        }
    }
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Refused {}

/// 用户名、密码握手换到的登录令牌：替人做事的程序拿它另开连接（[`Core::connect_login`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Login {
    /// 登录令牌。
    pub token: String,
    /// 什么时候过期。
    pub expires: String,
}

/// 照终端的样子连着的核心：一问一答。
pub struct Core {
    lines: BufReader<ReadHalf<Connection>>,
    write: WriteHalf<Connection>,
    /// 握手回的 `language`。
    language: Option<String>,
}

impl Core {
    /// 连核心（没在跑就照 `core` 拉起来）、出示本机令牌握手，报自己是 `kind` 这个头。
    ///
    /// # Errors
    ///
    /// 连不上、拉不起来，握手被拒、没回：[`Refused`]。
    pub async fn connect(root: &DataRoot, core: &CoreCommand, kind: &str) -> Result<Core, Refused> {
        let (connection, token) = miyu_ipc::connect_or_start(root, || core())
            .await
            .map_err(|error| Refused::local("unreachable", error))?;
        Core::shake(connection, &[("token", &token)], kind)
            .await
            .map(|(core, _)| core)
    }

    /// 连已经在跑的核心（不拉起）、出示本机令牌握手，报自己是 `kind` 这个头（施工 9-1 下：网页软件的 `serve` 起来时问
    /// 配置，不为这个拉起核心）。
    ///
    /// # Errors
    ///
    /// 核心没在跑、连不上，握手被拒、没回：[`Refused`]。
    pub async fn connect_running(root: &DataRoot, kind: &str) -> Result<Core, Refused> {
        let (connection, token) = miyu_ipc::connect(root)
            .await
            .map_err(|error| Refused::local("unreachable", error))?;
        Core::shake(connection, &[("token", &token)], kind)
            .await
            .map(|(core, _)| core)
    }

    /// 连已经在跑的核心（不拉起）、出示登录令牌 `login` 握手，报自己是 `kind` 这个头：替人做事的程序（接入通讯平台的页面服务，
    /// 它的设计 01 第九节第 2 条）照人的登录另开一条连接，不读本机令牌。
    ///
    /// # Errors
    ///
    /// 核心没在跑、连不上，登录令牌不认（`bad_login`）、没回：[`Refused`]。
    pub async fn connect_login(root: &DataRoot, login: &str, kind: &str) -> Result<Core, Refused> {
        let connection = miyu_ipc::connect_bare(root)
            .await
            .map_err(|error| Refused::local("unreachable", error))?;
        Core::shake(connection, &[("login", login)], kind)
            .await
            .map(|(core, _)| core)
    }

    /// 连已经在跑的核心（不拉起）、照访问密码 `password` 握手（施工 W-8 再补，接入通讯平台的页面登录框用；施工 SU-1 起没有
    /// 用户名）：不读本机令牌。交回连上的核心和换到的设备令牌（以后照它 [`Core::connect_login`]）。
    ///
    /// # Errors
    ///
    /// 核心没在跑、连不上；密码不对（`bad_password`）、还没设（`password_not_set`）、错多了被挡（`login_throttled`，带还要等
    /// 多久）；没回。
    pub async fn connect_password(
        root: &DataRoot,
        password: &str,
        kind: &str,
    ) -> Result<(Core, Login), Refused> {
        let connection = miyu_ipc::connect_bare(root)
            .await
            .map_err(|error| Refused::local("unreachable", error))?;
        let (core, shaken) = Core::shake(connection, &[("password", password)], kind).await?;
        let login = Login {
            token: shaken["login"]["token"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            expires: shaken["login"]["expires"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        };
        Ok((core, login))
    }

    /// 在连上的 `connection` 上出示凭据 `credentials`（`token` 本机令牌、`login` 设备令牌、`password` 访问密码）握手，报自己
    /// 是 `kind`。交回连上的核心和握手的回应。
    async fn shake(
        connection: Connection,
        credentials: &[(&str, &str)],
        kind: &str,
    ) -> Result<(Core, Value), Refused> {
        let (read, write) = tokio::io::split(connection);
        let mut core = Core {
            lines: BufReader::new(read),
            write,
            language: None,
        };
        let mut hello = json!({
            "protocol": [1, 1],
            "head": {"kind": kind, "version": env!("CARGO_PKG_VERSION")},
            "locale": locale(),
        });
        for (key, value) in credentials {
            hello[*key] = json!(value);
        }
        let shaken = core.call("hello", "hello", hello).await?;
        core.language = shaken["language"].as_str().map(str::to_string);
        Ok((core, shaken))
    }

    /// 握手回的 `language`：没回的是空的。
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    /// 发一条请求、等它的回应：交回 `result`，拒绝的交回原话。
    ///
    /// # Errors
    ///
    /// 写不出去、30 秒没回、核心断开，核心拒了：[`Refused`]，照 `reason` 分。
    pub async fn call(&mut self, id: &str, method: &str, params: Value) -> Result<Value, Refused> {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let sent = async {
            self.write
                .write_all(format!("{request}\n").as_bytes())
                .await?;
            self.write.flush().await
        };
        sent.await.map_err(|error| Refused::local("io", error))?;
        loop {
            let mut line = String::new();
            let read =
                tokio::time::timeout(Duration::from_secs(30), self.lines.read_line(&mut line))
                    .await
                    .map_err(|_| Refused::local("no_answer", "no answer"))?
                    .map_err(|error| Refused::local("io", error))?;
            if read == 0 {
                return Err(Refused::local("disconnected", "core disconnected"));
            }
            let Ok(reply) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if reply["id"] != json!(id) {
                continue;
            }
            return match reply.get("result") {
                Some(result) => Ok(result.clone()),
                None => Err(Refused::of(&reply["error"])),
            };
        }
    }
}

/// 这台机器的语言：`LC_ALL`、`LC_MESSAGES`、`LANG` 照先后（核心照它算 `auto` 的语言）。
pub fn locale() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}
