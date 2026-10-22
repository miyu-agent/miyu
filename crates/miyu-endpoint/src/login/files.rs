//! 凭据文件（施工 W-8）：`system/accounts.json`、各账号的 `logins.json`、`system/invites.json`（施工 Y-1 上）怎么读、改、
//! 写，系统日志记一条。调用的一方拿着 [`super::Identity`] 的锁，一件件改。
//!
//! 读不了、坏了的：当是空的，记一行 `WARN accounts not read`、`WARN logins not read`；写回时照现在磁盘上的字节的版本盖掉它。
//! 写不下的记一行 `WARN … not written`，回 `internal_error`。

use serde::Serialize;

use miyu_kernel::id::AccountId;
use miyu_kernel::time::Timestamp;
use miyu_store::config_file::{self, WriteError};
use miyu_store::{accounts, invites, logins};

use super::{TARGET, by, login_expires, random_hex};
use crate::Core;
use crate::refusal::Refusal;

/// 读 `system/accounts.json`：坏了的当空的。
pub(super) fn read_accounts(core: &Core) -> accounts::Read {
    let path = core.root.system().join(accounts::FILE);
    accounts::read(&path).unwrap_or_else(|error| {
        tracing::warn!(target: TARGET, error = %error, "accounts not read");
        accounts::Read {
            accounts: accounts::Accounts::default(),
            version: current(&path),
        }
    })
}

/// 读账号 `account` 的 `logins.json`：坏了的当空的。
pub(super) fn read_logins(core: &Core, account: &AccountId) -> logins::Read {
    let path = logins_path(core, account);
    logins::read(&path).unwrap_or_else(|error| {
        tracing::warn!(target: TARGET, error = %error, "logins not read");
        logins::Read {
            logins: logins::Logins::default(),
            version: current(&path),
        }
    })
}

/// 给管理员设用户名 `username`、密码哈希 `hashed`：交回是不是第一次设。
pub(super) fn put_account(
    core: &Core,
    username: &str,
    hashed: String,
    now: Timestamp,
) -> Result<bool, Refusal> {
    let read = read_accounts(core);
    let mut accounts = read.accounts;
    let first = accounts.of(core.admin.as_str()).is_none();
    accounts.put(accounts::Account {
        id: core.admin.as_str().to_string(),
        username: username.to_string(),
        password: hashed,
        changed: now,
        created: None,
        disabled: false,
    });
    let path = core.root.system().join(accounts::FILE);
    accounts::write(&path, &accounts, read.version.as_deref())
        .map_err(|error| not_written("accounts not written", &error))?;
    Ok(first)
}

/// 改 `system/accounts.json`：读出来交给 `change` 改，改好了写回。`change` 交回的错照原样往上报，什么都不写。
pub(super) fn change_accounts<T>(
    core: &Core,
    change: impl FnOnce(&mut accounts::Accounts) -> Result<T, Refusal>,
) -> Result<T, Refusal> {
    let read = read_accounts(core);
    let mut accounts = read.accounts;
    let out = change(&mut accounts)?;
    let path = core.root.system().join(accounts::FILE);
    accounts::write(&path, &accounts, read.version.as_deref())
        .map_err(|error| not_written("accounts not written", &error))?;
    Ok(out)
}

/// 登录令牌能落到的账号（施工 Y-1 上）：管理员，和 `accounts.json` 里没停用的成员。
pub(super) fn login_accounts(core: &Core) -> Vec<AccountId> {
    let accounts = read_accounts(core).accounts;
    let disabled = |id: &AccountId| accounts.of(id.as_str()).is_some_and(|each| each.disabled);
    std::iter::once(core.admin.clone())
        .chain(
            accounts
                .accounts
                .iter()
                .filter_map(|each| AccountId::parse(&each.id).ok()),
        )
        .filter(|id| !disabled(id))
        .fold(Vec::new(), |mut all, id| {
            if !all.contains(&id) {
                all.push(id);
            }
            all
        })
}

/// 读 `system/invites.json`：坏了的当空的。
pub(super) fn read_invites(core: &Core) -> invites::Read {
    let path = core.root.system().join(invites::FILE);
    invites::read(&path).unwrap_or_else(|error| {
        tracing::warn!(target: TARGET, error = %error, "invites not read");
        invites::Read {
            invites: invites::Invites::default(),
            version: current(&path),
        }
    })
}

/// 改 `system/invites.json`：同 [`change_accounts`]。
pub(super) fn change_invites<T>(
    core: &Core,
    change: impl FnOnce(&mut invites::Invites) -> Result<T, Refusal>,
) -> Result<T, Refusal> {
    let read = read_invites(core);
    let mut invites = read.invites;
    let out = change(&mut invites)?;
    let path = core.root.system().join(invites::FILE);
    invites::write(&path, &invites, read.version.as_deref())
        .map_err(|error| not_written("invites not written", &error))?;
    Ok(out)
}

/// 给账号 `account` 造一个登录令牌，记进它的 `logins.json`：交回令牌、它的哈希、过期的时刻。
pub(super) fn issue(
    core: &Core,
    account: &AccountId,
    now: Timestamp,
) -> Result<(String, String, Timestamp), Refusal> {
    let token = random_hex()?;
    let hash = logins::digest(&token);
    let expires = login_expires(now);
    let read = read_logins(core, account);
    let mut logins = read.logins;
    logins.add(
        logins::Login {
            created: now,
            expires,
            hash: hash.clone(),
        },
        now,
    );
    logins::write(
        &logins_path(core, account),
        &logins,
        read.version.as_deref(),
    )
    .map_err(|error| not_written("logins not written", &error))?;
    Ok((token, hash, expires))
}

/// 作废账号 `account` 的登录令牌：`one` 是那一个的哈希，空的是全部。交回作废了几个；一个都没有的不写文件。
pub(super) fn revoke(
    core: &Core,
    account: &AccountId,
    one: Option<&str>,
) -> Result<usize, Refusal> {
    let read = read_logins(core, account);
    let mut logins = read.logins;
    let count = match one {
        Some(hash) => usize::from(logins.remove(hash)),
        None => logins.clear(),
    };
    if count > 0 {
        logins::write(
            &logins_path(core, account),
            &logins,
            read.version.as_deref(),
        )
        .map_err(|error| not_written("logins not written", &error))?;
    }
    Ok(count)
}

/// `account.password_set` 的 `body`：设成的用户名、是不是第一次。
#[derive(Serialize)]
struct PasswordSet<'a> {
    username: &'a str,
    first: bool,
}

/// 系统日志记一条 `account.password_set`。写不进的记一行 `WARN`，不往上报。
pub(super) fn journal(core: &Core, username: &str, first: bool, at: Timestamp) {
    record(
        core,
        &core.admin,
        at,
        "account.password_set",
        &PasswordSet { username, first },
    );
}

/// 系统日志记一条 `kind`（施工 Y-1 上起账号的变化都记）：`who` 是做这件事的账号。写不进的记一行 `WARN`，不往上报。
pub(super) fn record(
    core: &Core,
    who: &AccountId,
    at: Timestamp,
    kind: &str,
    body: &impl Serialize,
) {
    let path = core.config().places.system_journal.clone();
    let shown = format!("system/{}", miyu_store::journal::FILE);
    crate::config::journal::record(&path, &shown, at, by(who), None, kind, body);
}

/// 账号 `account` 的 `logins.json` 在哪。
fn logins_path(core: &Core, account: &AccountId) -> std::path::PathBuf {
    core.root.account_dir(account).join(logins::FILE)
}

/// 磁盘上现在那份的版本：坏了的文件写回时照它盖。没有的、读不了的是空的。
fn current(path: &std::path::Path) -> Option<String> {
    std::fs::read(path)
        .ok()
        .map(|bytes| config_file::version(&bytes))
}

/// 写不下：记一行，回 `internal_error`。
fn not_written(what: &'static str, error: &WriteError) -> Refusal {
    tracing::warn!(target: TARGET, error = %error, "{what}");
    Refusal::INTERNAL
}
