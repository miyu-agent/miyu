//! 身份（`docs/blueprint/web-module.md`「怎么走」第一条，施工 W-8）：本机的浏览器经网页软件连核心，凭据由核心在握手时验。
//!
//! - 一次性码（`account.setup_code`）：只给出示本机令牌连上的连接；5 分钟、一次；只在内存里，最多 16 个。
//! - 用一次性码连上的连接只能设用户名和密码（`account.setup`）：设好了写 `system/accounts.json`，这个账号以前的登录令牌
//!   全部作废，造一个新的交回；这个连接从此是完整的管理员。
//! - 用户名、密码握手：对上了造一个登录令牌交回；同一个用户名 60 秒内试 5 次（验着的也算，对了清零），剩下的时间里先拒、
//!   不验（施工 5-20）。
//! - 登录令牌握手：哈希在这个账号的 `logins.json` 里、没过期。
//! - `account.logout`：作废一个、全部；用登录令牌、密码连着的连接收到作废就断开（[`Identity::revoked`]）。
//! - 交接票 `login.ticket`、`login.redeem`（接入通讯平台要的）在 [`ticket`]。
//! - 多账号（施工 Y-1 上）：邀请码在 [`invites`]，注册、列出、停用在 [`members`]。用户名、密码照用户名找账号，登录令牌照令牌
//!   找属于哪个账号（一个个账号的 `logins.json` 找）；停用了的进不来。连接带着认出来的账号。
//!
//! 一次性码、密码、登录令牌一个字都不进运行日志。凭据文件的读写在 `miyu-store`（`accounts.rs`、`logins.rs`），这里排着队
//! 一件件改（[`Identity`] 的锁），argon2 放进阻塞线程。

pub(crate) mod admins;
mod files;
pub(crate) use files::read_groups;
pub(crate) mod invites;
pub(crate) mod members;
mod oobe;
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

/// 数登录失败的窗口、窗口里错几次就先拒（设计 06 U4 的初值）。
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
    /// 一次性码：只能设密码。
    Code,
    /// 邀请码（施工 Y-1 上）：只能注册，带着码的哈希。
    Invite(String),
    /// 登录令牌：带着它的哈希和它属于哪个账号。
    Login(String, AccountId),
    /// 用户名、密码：带着这次造的登录令牌的哈希和账号。
    Password(String, AccountId),
    /// 核心亲手拉起的扩展（施工 9-4 上）：标准输入输出是核心给的，不看凭据。
    Spawned,
}

impl Via {
    /// 运行日志里的写法。
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Via::Token => "token",
            Via::Code => "code",
            Via::Invite(_) => "invite",
            Via::Login(..) => "login",
            Via::Password(..) => "password",
            Via::Spawned => "spawned",
        }
    }

    /// 它靠的那个登录令牌的哈希：用登录令牌、密码连上的才有。
    pub(crate) fn login(&self) -> Option<&str> {
        match self {
            Via::Login(hash, _) | Via::Password(hash, _) => Some(hash),
            Via::Token | Via::Code | Via::Invite(_) | Via::Spawned => None,
        }
    }

    /// 凭据认出来的账号（施工 Y-1 上）：用登录令牌、密码连上的才有；本机令牌、扩展的照连接原来的。
    pub(crate) fn account(&self) -> Option<&AccountId> {
        match self {
            Via::Login(_, account) | Via::Password(_, account) => Some(account),
            Via::Token | Via::Code | Via::Invite(_) | Via::Spawned => None,
        }
    }
}

/// 作废了哪些登录令牌：连着的连接照它断开。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Revoked {
    /// 这个账号全部的（施工 Y-1 上起带账号：只断这个账号的连接）。
    All(AccountId),
    /// 设置那一段注册了机主（施工 Y-12 中上）：设置那一段的连接全部断开，头重连进照机主起的那一份。
    Everyone,
    /// 哈希是它的那一个。
    One(String),
}

/// 一个还没用的一次性码。
struct Code {
    value: String,
    expires: Instant,
}

/// 核心一份的：一次性码、登录失败的计数、作废的广播，改凭据文件排队的锁。
pub(crate) struct Identity {
    codes: Mutex<Vec<Code>>,
    failures: Mutex<BTreeMap<String, Vec<Instant>>>,
    revoked: broadcast::Sender<Revoked>,
    files: tokio::sync::Mutex<()>,
    code_ttl: Duration,
    /// 交接票（`ticket.rs`）。
    tickets: ticket::Tickets,
    /// 设置那一段（施工 Y-12 中上，`oobe.rs`）：还没有机主时有，注册了机主时响一下。
    setup: Option<Arc<tokio::sync::Notify>>,
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
            setup: None,
        }
    }

    /// 收作废的：用登录令牌、密码连上的连接一个一份。
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
    let first = files::read_accounts(core)
        .accounts
        .of(core.admin.as_str())
        .is_none();
    Ok(json!({"code": code, "expires": stamp(expires), "first": first}))
}

/// `account.setup` 的参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetupParams {
    username: String,
    password: String,
}

/// `account.setup`：用一次性码连上的设用户名、密码；设置那一段本机令牌的连接也能，注册的是机主（施工 Y-12 中上，`oobe.rs`）。
/// 交回回应和这个连接以后怎么算。
pub(crate) async fn setup(
    core: &Arc<Core>,
    via: &Via,
    params: Value,
) -> Result<(Value, Via), Refusal> {
    let setting_up = core.setting_up();
    if !(*via == Via::Code || setting_up && *via == Via::Token) {
        return Err(Refusal::BAD_PARAMS);
    }
    let params: SetupParams = serde_json::from_value(params).map_err(|_| Refusal::BAD_PARAMS)?;
    if AccountId::parse(&params.username).is_err() || !good_password(&params.password) {
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
    if setting_up {
        return oobe::owner(core, &params.username, hashed).await;
    }
    let _queue = core.identity.files.lock().await;
    let now = crate::sessions::now();
    let first = files::put_account(core, &params.username, hashed, now)?;
    tracing::info!(target: TARGET, first, "password set");
    let revoked = files::revoke(core, &core.admin, None)?;
    if revoked > 0 {
        tracing::info!(target: TARGET, count = revoked, "logins revoked");
    }
    // 连着的旧登录一起断开（重设的时候把以前的浏览器踢出去）。
    announce(&core.identity, Revoked::All(core.admin.clone()));
    let (token, hash, expires) = files::issue(core, &core.admin, now)?;
    tracing::info!(target: TARGET, via = "setup", "login issued");
    files::journal(core, &params.username, first, now);
    let reply = json!({
        "username": params.username,
        "login": {"expires": expires, "token": token},
    });
    Ok((reply, Via::Login(hash, core.admin.clone())))
}

/// 用户名 `user`、密码 `password` 握手：对上了交回登录令牌、它的过期时刻和这个连接怎么算。
pub(crate) async fn password(
    core: &Arc<Core>,
    user: &str,
    password: String,
) -> Result<(Value, Via), Refusal> {
    if let Err(wait) = admit(&core.identity, user) {
        return Err(Refusal::login_throttled(millis(wait).max(1).unsigned_abs()));
    }
    // 停用了的照样验密码：密码对了才说停用了（施工 Y-1 上补，2026-10-22 项目主人验收时要的），不对的照旧 `bad_password`，
    // 不透露停没停。
    let found = files::read_accounts(core)
        .accounts
        .find(user)
        .and_then(|account| {
            let id = AccountId::parse(&account.id).ok()?;
            Some(((id, account.disabled), account.password.clone()))
        });
    let (account, phc) = found.unzip();
    let right = tokio::task::spawn_blocking(move || match phc {
        Some(phc) => accounts::verify(&phc, &password),
        // 没有这个用户名也验一次，和密码不对花一样的时间。
        None => {
            let _ = accounts::verify(stand_in(), &password);
            false
        }
    })
    .await
    .unwrap_or(false);
    // 错了的进来时已经记过（`admit`）。
    let Some((account, disabled)) = account.filter(|_| right) else {
        return Err(Refusal::BAD_PASSWORD);
    };
    if disabled {
        return Err(Refusal::ACCOUNT_DISABLED);
    }
    core.identity
        .failures
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(user);
    let _queue = core.identity.files.lock().await;
    let (token, hash, expires) = files::issue(core, &account, crate::sessions::now())?;
    tracing::info!(target: TARGET, via = "password", "login issued");
    Ok((
        json!({"expires": expires, "token": token}),
        Via::Password(hash, account),
    ))
}

/// 登录令牌 `token` 握手：认得的交回这个连接怎么算。
pub(crate) fn login(core: &Core, token: &str) -> Option<Via> {
    let hash = miyu_store::logins::digest(token);
    let now = crate::sessions::now();
    // 照令牌找属于哪个账号（施工 Y-1 上）：管理员、没停用的成员一个个找。
    files::login_accounts(core)
        .into_iter()
        .find(|account| files::read_logins(core, account).logins.valid(&hash, now))
        .map(|account| Via::Login(hash, account))
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
    // 作废的是这个连接自己账号的（施工 Y-1 上）：本机令牌连上的是管理员。
    let account = via.account().cloned().unwrap_or_else(|| core.admin.clone());
    let _queue = core.identity.files.lock().await;
    let revoked = files::revoke(core, &account, one.as_deref())?;
    tracing::info!(target: TARGET, count = revoked, "logins revoked");
    announce(
        &core.identity,
        match one {
            Some(hash) => Revoked::One(hash),
            None => Revoked::All(account),
        },
    );
    Ok((json!({"revoked": revoked}), via.login().is_some()))
}

/// 告诉连着的连接哪些登录令牌作废了；一个都没在听的不要紧。
fn announce(identity: &Identity, revoked: Revoked) {
    identity.revoked.send(revoked).unwrap_or(0);
}

/// 这个用户名能不能再试：60 秒内已经试了 5 次的，交回还要等多久（等到最早那一次满 60 秒）；能试的当场先记一次失败，验对了
/// 再清零。先查、验完才记的话，同时进来的握手都看到「还没错满」、都去验，60 秒 5 次挡不住并发，每个还各占一份 argon2 的
/// 内存（施工 5-20）：查和记在同一把锁里。
fn admit(identity: &Identity, user: &str) -> Result<(), Duration> {
    let mut failures = identity
        .failures
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let now = Instant::now();
    let tries = failures.entry(user.to_string()).or_default();
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

/// 没有这个用户名时陪着验的一串：照同样的参数算的，谁的密码都验不过。
fn stand_in() -> &'static str {
    static STAND_IN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    STAND_IN.get_or_init(|| accounts::hash("miyu stand-in password").unwrap_or_default())
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

/// 在凭据文件那把锁里改组表（施工 Y-5 下上）：扩展审批改 `ext.<能力>` 组、`account.set_admin` 改 `admin` 组，一件件来。
pub(crate) async fn change_groups<T>(
    core: &Core,
    change: impl FnOnce(&mut miyu_store::groups::Groups) -> Result<T, Refusal>,
) -> Result<T, Refusal> {
    let _queue = core.identity.files.lock().await;
    files::change_groups(core, change)
}
