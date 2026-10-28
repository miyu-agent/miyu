//! 凭据文件（施工 W-8，施工 SU-1 起只有主人一个）：`system/accounts.json` 主人那一行（访问密码的哈希）、主人的 `logins.json`
//! （设备令牌）怎么读、改、写，系统日志记一条。调用的一方拿着 [`super::Identity`] 的锁，一件件改。
//!
//! 读不了、坏了的：当是空的，记一行 `WARN accounts not read`、`WARN logins not read`；写回时照现在磁盘上的字节的版本盖掉它。
//! 写不下的记一行 `WARN … not written`，回 `internal_error`。

use serde::Serialize;

use miyu_kernel::id::AccountId;
use miyu_kernel::time::Timestamp;
use miyu_store::config_file::{self, WriteError};
use miyu_store::{accounts, logins};

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

/// 读主人的 `logins.json`：坏了的当空的。
pub(super) fn read_logins(core: &Core) -> logins::Read {
    let path = logins_path(core);
    logins::read(&path).unwrap_or_else(|error| {
        tracing::warn!(target: TARGET, error = %error, "logins not read");
        logins::Read {
            logins: logins::Logins::default(),
            version: current(&path),
        }
    })
}

/// 给主人设访问密码，哈希是 `hashed`：交回是不是第一次设。
pub(super) fn put_password(core: &Core, hashed: String, now: Timestamp) -> Result<bool, Refusal> {
    let read = read_accounts(core);
    let mut accounts = read.accounts;
    let was = accounts.of(core.admin.as_str());
    let first = was.is_none_or(|was| was.password.is_empty());
    let created = was.and_then(|was| was.created);
    accounts.put(accounts::Account {
        id: core.admin.as_str().to_string(),
        username: core.admin.as_str().to_string(),
        password: hashed,
        changed: now,
        created,
        disabled: false,
        owner: true,
    });
    let path = core.root.system().join(accounts::FILE);
    accounts::write(&path, &accounts, read.version.as_deref())
        .map_err(|error| not_written("accounts not written", &error))?;
    Ok(first)
}

/// 造一个设备令牌，记进主人的 `logins.json`：交回令牌、它的哈希、过期的时刻。
pub(super) fn issue(core: &Core, now: Timestamp) -> Result<(String, String, Timestamp), Refusal> {
    let token = random_hex()?;
    let hash = logins::digest(&token);
    let expires = login_expires(now);
    let read = read_logins(core);
    let mut logins = read.logins;
    logins.add(
        logins::Login {
            created: now,
            expires,
            hash: hash.clone(),
        },
        now,
    );
    logins::write(&logins_path(core), &logins, read.version.as_deref())
        .map_err(|error| not_written("logins not written", &error))?;
    Ok((token, hash, expires))
}

/// 作废设备令牌：`one` 是那一个的哈希，空的是全部。交回作废了几个；一个都没有的不写文件。
pub(super) fn revoke(core: &Core, one: Option<&str>) -> Result<usize, Refusal> {
    let read = read_logins(core);
    let mut logins = read.logins;
    let count = match one {
        Some(hash) => usize::from(logins.remove(hash)),
        None => logins.clear(),
    };
    if count > 0 {
        logins::write(&logins_path(core), &logins, read.version.as_deref())
            .map_err(|error| not_written("logins not written", &error))?;
    }
    Ok(count)
}

/// `account.password_set` 的 `body`：是不是第一次设。
#[derive(Serialize)]
struct PasswordSet {
    first: bool,
}

/// 系统日志记一条 `account.password_set`。写不进的记一行 `WARN`，不往上报。
pub(super) fn journal(core: &Core, first: bool, at: Timestamp) {
    record(
        core,
        &core.admin,
        at,
        "account.password_set",
        &PasswordSet { first },
    );
}

/// 系统日志记一条 `kind`：`who` 是做这件事的账号。写不进的记一行 `WARN`，不往上报。
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

/// 主人的 `logins.json` 在哪。
fn logins_path(core: &Core) -> std::path::PathBuf {
    core.root.account_dir(&core.admin).join(logins::FILE)
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
