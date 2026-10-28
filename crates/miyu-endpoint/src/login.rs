//! 身份（`docs/blueprint/web-module.md`「怎么走」第一条，施工 W-8；施工 SU-1 起只有一个主人）：本机的头凭本机令牌；网页从局域网
//! 连时凭访问密码换的设备令牌。
//!
//! - 一次性码（`account.setup_code`）：只给出示本机令牌连上的连接；5 分钟、一次；只在内存里，最多 16 个。网页第一次设访问密码用。
//! - `account.setup {"password"}`：本机令牌、一次性码连上的设、改访问密码，写进 `system/accounts.json` 主人那一行；以前的设备令牌
//!   全部作废。一次性码连上的另交回一个新的设备令牌，这个连接从此是登录了的。
//! - 访问密码握手：对上了造一个设备令牌（登录令牌）交回；照来源（网页软件转来的 `connection.remote` 的地址，本机来的算一份）60 秒内
//!   试 5 次，剩下的时间里先拒、不验（施工 5-20）。还没设访问密码的一律 `password_not_set`。
//! - 设备令牌握手：哈希在主人的 `logins.json` 里、没过期。
//! - `account.logout`：作废一个、全部；用设备令牌、密码连着的连接收到作废就断开（[`Identity::revoked`]）。
//! - 交接票 `login.ticket`、`login.redeem`（接入通讯平台、`miyu web` 要的）在 [`ticket`]。
//!
//! 一次性码、密码、设备令牌一个字都不进运行日志。凭据文件的读写在 `miyu-store`（`accounts.rs`、`logins.rs`），这里排着队
//! 一件件改（[`Identity`] 的锁），argon2 放进阻塞线程。

pub(crate) mod avatar;
mod files;
pub(crate) use files::read_groups;
pub(crate) mod ticket;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::broadcast;

use miyu_kernel::id::AccountId;
use miyu_kernel::origin::{By, Person};
use miyu_kernel::time::Timestamp;
use miyu_store::accounts;

use crate::Core;
use crate::refusal::Refusal;

/// 运行日志的目标。
const TARGET: &str = "miyu::endpoint";

/// 一次性码多久有效（设计 21 X6）。
pub(crate) const CODE_TTL: Duration = Duration::from_secs(5 * 60);

/// 同时最多几个一次性码：多了丢最早的。
const MOST_CODES: usize = 16;

/// 登录令牌多久有效（设计 06 U4）。
const LOGIN_DAYS: i64 = 30;

/// 数登录失败的窗口、窗口里错几次就先拒（设计 06 U4 的初值）：照来源数。
const WINDOW: Duration = Duration::from_secs(60);
const TRIES: usize = 5;

/// 密码最短、最长几个字节（「起草时定的」第 45 条）。
const SHORTEST: usize = 8;
const LONGEST: usize = 1024;

/// 这个连接是怎么认出来的（握手时定）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Via {
    /// 本机令牌：本机的头。
    Token,
    /// 一次性码：只能设访问密码。
    Code,
    /// 设备令牌（登录令牌）：带着它的哈希。
    Login(String),
    /// 访问密码：带着这次造的设备令牌的哈希。
    Password(String),
    /// 核心亲手拉起的扩展（施工 9-4 上）：标准输入输出是核心给的，不看凭据。
    Spawned,
}

impl Via {
    /// 运行日志里的写法。
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Via::Token => "token",
            Via::Code => "code",
            Via::Login(..) => "login",
            Via::Password(..) => "password",
            Via::Spawned => "spawned",
        }
    }

    /// 它靠的那个设备令牌的哈希：用设备令牌、密码连上的才有。
    pub(crate) fn login(&self) -> Option<&str> {
        match self {
            Via::Login(hash) | Via::Password(hash) => Some(hash),
            Via::Token | Via::Code | Via::Spawned => None,
        }
    }
}

/// 作废了哪些登录令牌：连着的连接照它断开。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Revoked {
    /// 全部的。
    All,
    /// 哈希是它的那一个。
    One(String),
}

/// 一个还没用的一次性码。
struct Code {
    value: String,
    expires: Instant,
}

/// 核心一份的：一次性码、登录失败的计数（照来源）、作废的广播，改凭据文件排队的锁。
pub(crate) struct Identity {
    codes: Mutex<Vec<Code>>,
    failures: Mutex<BTreeMap<String, Vec<Instant>>>,
    revoked: broadcast::Sender<Revoked>,
    files: tokio::sync::Mutex<()>,
    code_ttl: Duration,
    /// 交接票（`ticket.rs`）。
    tickets: ticket::Tickets,
}

impl Identity {
    /// 一次性码 `code_ttl` 有效。
    pub(crate) fn new(code_ttl: Duration) -> Identity {
        Identity {
            codes: Mutex::new(Vec::new()),
            failures: Mutex::new(BTreeMap::new()),
            revoked: broadcast::channel(64).0,
            files: tokio::sync::Mutex::new(()),
            code_ttl,
            tickets: ticket::Tickets::default(),
        }
    }

    /// 收作废的：用设备令牌、密码连上的连接一个一份。
    pub(crate) fn revoked(&self) -> broadcast::Receiver<Revoked> {
        self.revoked.subscribe()
    }

    /// 用掉一次性码 `code`：在、没过期的交回真，当场作废。
    pub(crate) fn take_code(&self, code: &str) -> bool {
        let mut codes = self.codes.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        codes.retain(|each| each.expires > now);
        let before = codes.len();
        codes.retain(|each| !crate::hello::same(&each.value, code));
        codes.len() < before
    }
}

/// `account.setup_code`：只给出示本机令牌的连接。
pub(crate) fn setup_code(core: &Core, via: &Via) -> Result<Value, Refusal> {
    if *via != Via::Token {
        return Err(Refusal::LOCAL_ONLY);
    }
    let code = random_hex()?;
    let identity = &core.identity;
    let expires = crate::sessions::now().unix_millis() + millis(identity.code_ttl);
    {
        let mut codes = identity
            .codes
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        codes.retain(|each| each.expires > now);
        codes.push(Code {
            value: code.clone(),
            expires: now + identity.code_ttl,
        });
        while codes.len() > MOST_CODES {
            codes.remove(0);
        }
    }
    let first = !password_set(core);
    Ok(json!({"code": code, "expires": stamp(expires), "first": first}))
}

/// `account.setup` 的参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetupParams {
    password: String,
}

/// `account.setup`：本机令牌、一次性码连上的设、改访问密码。以前的设备令牌全部作废、连着的断开；一次性码连上的另交回一个新的
/// 设备令牌，这个连接从此算登录了。交回回应和这个连接以后怎么算（没换的照旧）。
pub(crate) async fn setup(
    core: &Arc<Core>,
    via: &Via,
    params: Value,
) -> Result<(Value, Option<Via>), Refusal> {
    if !matches!(via, Via::Code | Via::Token) {
        return Err(Refusal::LOCAL_ONLY);
    }
    let params: SetupParams = serde_json::from_value(params).map_err(|_| Refusal::BAD_PARAMS)?;
    if !good_password(&params.password) {
        return Err(Refusal::BAD_PARAMS);
    }
    let password = params.password;
    let hashed = tokio::task::spawn_blocking(move || accounts::hash(&password))
        .await
        .map_err(|_| Refusal::INTERNAL)?
        .map_err(|error| {
            tracing::warn!(target: TARGET, error = %error, "password not hashed");
            Refusal::INTERNAL
        })?;
    let _queue = core.identity.files.lock().await;
    let now = crate::sessions::now();
    let first = files::put_password(core, hashed, now)?;
    tracing::info!(target: TARGET, first, "password set");
    let revoked = files::revoke(core, None)?;
    if revoked > 0 {
        tracing::info!(target: TARGET, count = revoked, "logins revoked");
    }
    // 连着的旧登录一起断开（改密码的时候把以前的设备踢出去）。
    announce(&core.identity, Revoked::All);
    files::journal(core, first, now);
    if *via == Via::Token {
        return Ok((json!({}), None));
    }
    let (token, hash, expires) = files::issue(core, now)?;
    tracing::info!(target: TARGET, via = "setup", "login issued");
    let reply = json!({"login": {"expires": expires, "token": token}});
    Ok((reply, Some(Via::Login(hash))))
}

/// 设过访问密码没有。
pub(crate) fn password_set(core: &Core) -> bool {
    files::read_accounts(core)
        .accounts
        .of(core.admin.as_str())
        .is_some_and(|owner| !owner.password.is_empty())
}

/// 访问密码 `password` 握手，`source` 是来源（网页软件转来的地址；本机来的没有）：对上了交回设备令牌、它的过期时刻和这个连接
/// 怎么算。
pub(crate) async fn password(
    core: &Arc<Core>,
    source: Option<&str>,
    password: String,
) -> Result<(Value, Via), Refusal> {
    let source = source.unwrap_or("local").to_string();
    if let Err(wait) = admit(&core.identity, &source) {
        return Err(Refusal::login_throttled(millis(wait).max(1).unsigned_abs()));
    }
    let phc = files::read_accounts(core)
        .accounts
        .of(core.admin.as_str())
        .map(|owner| owner.password.clone())
        .filter(|phc| !phc.is_empty());
    let Some(phc) = phc else {
        return Err(Refusal::PASSWORD_NOT_SET);
    };
    let right = tokio::task::spawn_blocking(move || accounts::verify(&phc, &password))
        .await
        .unwrap_or(false);
    // 错了的进来时已经记过（`admit`）。
    if !right {
        return Err(Refusal::BAD_PASSWORD);
    }
    core.identity
        .failures
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&source);
    let _queue = core.identity.files.lock().await;
    let (token, hash, expires) = files::issue(core, crate::sessions::now())?;
    tracing::info!(target: TARGET, via = "password", "login issued");
    Ok((
        json!({"expires": expires, "token": token}),
        Via::Password(hash),
    ))
}

/// 设备令牌 `token` 握手：认得的交回这个连接怎么算。
pub(crate) fn login(core: &Core, token: &str) -> Option<Via> {
    let hash = miyu_store::logins::digest(token);
    let now = crate::sessions::now();
    files::read_logins(core)
        .logins
        .valid(&hash, now)
        .then_some(Via::Login(hash))
}

/// `account.logout` 的参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LogoutParams {
    #[serde(default)]
    all: bool,
}

/// `account.logout`：交回回应，和这个连接回完要不要断开。
pub(crate) async fn logout(
    core: &Arc<Core>,
    via: &Via,
    params: Value,
) -> Result<(Value, bool), Refusal> {
    let params: LogoutParams = serde_json::from_value(params).map_err(|_| Refusal::BAD_PARAMS)?;
    let one = match (params.all, via.login()) {
        (true, _) => None,
        (false, Some(hash)) => Some(hash.to_string()),
        (false, None) => return Err(Refusal::BAD_PARAMS),
    };
    let _queue = core.identity.files.lock().await;
    let revoked = files::revoke(core, one.as_deref())?;
    tracing::info!(target: TARGET, count = revoked, "logins revoked");
    announce(
        &core.identity,
        match one {
            Some(hash) => Revoked::One(hash),
            None => Revoked::All,
        },
    );
    Ok((json!({"revoked": revoked}), via.login().is_some()))
}

/// 告诉连着的连接哪些登录令牌作废了；一个都没在听的不要紧。
fn announce(identity: &Identity, revoked: Revoked) {
    identity.revoked.send(revoked).unwrap_or(0);
}

/// 这个来源能不能再试：60 秒内已经试了 5 次的，交回还要等多久（等到最早那一次满 60 秒）；能试的当场先记一次失败，验对了
/// 再清零。先查、验完才记的话，同时进来的握手都看到「还没错满」、都去验，60 秒 5 次挡不住并发，每个还各占一份 argon2 的
/// 内存（施工 5-20）：查和记在同一把锁里。
fn admit(identity: &Identity, source: &str) -> Result<(), Duration> {
    let mut failures = identity
        .failures
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let now = Instant::now();
    let tries = failures.entry(source.to_string()).or_default();
    tries.retain(|at| now.duration_since(*at) < WINDOW);
    if tries.len() >= TRIES {
        return Err(WINDOW - now.duration_since(tries[tries.len() - TRIES]));
    }
    tries.push(now);
    Ok(())
}

/// 密码写得合不合：8 到 1024 个字节，不能全是空白。
fn good_password(password: &str) -> bool {
    (SHORTEST..=LONGEST).contains(&password.len()) && !password.trim().is_empty()
}

/// 32 个系统给的随机字节，写成 64 位小写十六进制。
pub(crate) fn random_hex() -> Result<String, Refusal> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| {
        tracing::warn!(target: TARGET, error = %error, "no random bytes");
        Refusal::INTERNAL
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// 时刻写成协议上的样子。
fn stamp(unix_millis: i64) -> Value {
    Timestamp::from_unix_millis(unix_millis).map_or(Value::Null, |at| json!(at))
}

/// 时长的毫秒数。
fn millis(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

/// 登录令牌从 `now` 起什么时候过期。
fn login_expires(now: Timestamp) -> Timestamp {
    Timestamp::from_unix_millis(now.unix_millis() + LOGIN_DAYS * 86_400_000).unwrap_or(now)
}

/// 记进系统日志的人：账号 `account`。
fn by(account: &AccountId) -> By {
    By::Person(Person::new(account.clone()))
}

/// 在凭据文件那把锁里改组表（施工 Y-5 下上）：扩展审批改 `ext.<能力>` 组，一件件来。
pub(crate) async fn change_groups<T>(
    core: &Core,
    change: impl FnOnce(&mut miyu_store::groups::Groups) -> Result<T, Refusal>,
) -> Result<T, Refusal> {
    let _queue = core.identity.files.lock().await;
    files::change_groups(core, change)
}

/// `account` 是不是主人（施工 SU-1：只有一个人，以前的「管理员」就是他）。包的账号不是。
pub(crate) fn is_owner(core: &Core, account: &AccountId) -> bool {
    *account == core.admin
}
