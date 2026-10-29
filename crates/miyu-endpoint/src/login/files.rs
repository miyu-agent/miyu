//! 凭据文件（施工 W-8，施工 SU-6 上起只有一份）：`access.json`（访问密码的哈希）、`logins.json`（设备令牌）怎么读、改、写，
//! 日志记一条。调用的一方拿着 [`super::Identity`] 的锁，一件件改。
//!
//! 读不了、坏了的：当是空的，记一行 `WARN access not read`、`WARN logins not read`；写回时照现在磁盘上的字节的版本盖掉它。
//! 写不下的记一行 `WARN … not written`，回 `internal_error`。

use serde::Serialize;

use miyu_kernel::time::Timestamp;
use miyu_store::config_file::{self, WriteError};
use miyu_store::{access, logins};

use super::{TARGET, by, login_expires, random_hex};
use crate::Core;
use crate::refusal::Refusal;

/// 读 `access.json`：坏了的当空的。
pub(super) fn read_access(core: &Core) -> access::Read {
    let path = core.root.path().join(access::FILE);
    access::read(&path).unwrap_or_else(|error| {
        tracing::warn!(target: TARGET, error = %error, "access not read");
        access::Read {
            access: access::Access::default(),
            version: current(&path),
        }
    })
}

/// 读 `logins.json`：坏了的当空的。
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

/// 设访问密码，哈希是 `hashed`：交回是不是第一次设。
pub(super) fn put_password(core: &Core, hashed: String, now: Timestamp) -> Result<bool, Refusal> {
    let read = read_access(core);
    let first = read.access.password.is_empty();
    let access = access::Access {
        password: hashed,
        changed: Some(now),
        ..read.access
    };
    let path = core.root.path().join(access::FILE);
    access::write(&path, &access, read.version.as_deref())
        .map_err(|error| not_written("access not written", &error))?;
    Ok(first)
}

/// 造一个设备令牌，记进 `logins.json`：交回令牌、它的哈希、过期的时刻。
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

/// 日志记一条 `account.password_set`。写不进的记一行 `WARN`，不往上报。
pub(super) fn journal(core: &Core, first: bool, at: Timestamp) {
    record(core, at, "account.password_set", &PasswordSet { first });
}

/// 日志记一条 `kind`。写不进的记一行 `WARN`，不往上报。
pub(super) fn record(core: &Core, at: Timestamp, kind: &str, body: &impl Serialize) {
    let path = core.config().places.journal.clone();
    let shown = miyu_store::journal::FILE.to_string();
    crate::config::journal::record(&path, &shown, at, by(), None, kind, body);
}

/// `logins.json` 在哪。
fn logins_path(core: &Core) -> std::path::PathBuf {
    core.root.path().join(logins::FILE)
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
