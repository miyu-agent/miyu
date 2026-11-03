//! 头自己的设置（施工 HP-1，`docs/blueprint/protocol.md`「头自己的设置」）：`prefs.get`、`prefs.set`，和 `prefs` 流的订阅要的那一份。
//!
//! 网页原来把皮肤、配色、主题这些存在浏览器里，浏览器照地址分，换个地址、换台设备就是另一份；存进数据根，跟着人走。只有装着的
//! 界面包有（第 1 条），文件是它状态目录里的 `prefs.json`（`miyu_store::prefs`）。改、写、推在 [`Prefs`] 的一把锁里排着做，推的
//! 先后就是写的先后；推给订着这个包的别的连接，改的那个不推（转发任务在 `subscriptions/prefs.rs`）。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::sync::broadcast;

use miyu_config::package::PackageKind;
use miyu_store::config_file::WriteError;
use miyu_store::prefs as file;

use crate::Core;
use crate::hello::Caller;
use crate::refusal::Refusal;

/// 整份写成紧凑的 JSON 最多多少字节（第 4 条）：设置是几十个开关、几个名字，64 KiB 绰绰有余；再大的是拿它存别的东西了。
const LIMIT: usize = 64 * 1024;

/// 撞上手改（替换前再读一次对不上）从头再来最多几次：人手改设置文件本来就少见，连撞三次的当写不进。
const TRIES: usize = 3;

/// 推送的广播最多攒几份：一个订阅读得慢、攒过这么多就掉队，推 `resync`。设置是人点出来的，攒不了几份。
const BACKLOG: usize = 16;

/// 核心一份：改的排着做，改了广播出去。
#[derive(Debug)]
pub(crate) struct Prefs {
    /// 改、写、推排着做：两个头同时改同一个包的，后一个照前一个写好的并。
    lock: Mutex<()>,
    /// 改了的一份，订着的连接各自挑。
    changes: broadcast::Sender<Arc<Changed>>,
}

impl Default for Prefs {
    fn default() -> Prefs {
        let (changes, _) = broadcast::channel(BACKLOG);
        Prefs {
            lock: Mutex::new(()),
            changes,
        }
    }
}

/// 一次改动：推给订着这个包的别的连接。
#[derive(Debug)]
pub(crate) struct Changed {
    /// 哪个包的设置。
    pub(crate) package: String,
    /// 改的那个连接：不推给它（[`crate::reverse::Peer::writes_to`]）。
    pub(crate) from: crate::reverse::Peer,
    /// 写好的 `prefs.changed` 那一行。
    pub(crate) line: String,
}

/// `prefs.get` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct GetParams {
    /// 界面包的编号。
    package: String,
}

/// `prefs.set` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct SetParams {
    /// 界面包的编号。
    package: String,
    /// 改的几个顶层的键：值整个换上，`null` 的删掉。
    values: Map<String, Value>,
}

/// `prefs.get`：整份，没有的是 `{}`。
///
/// # Errors
///
/// 不是装着的界面包（[`file_of`]）；文件读不了。
pub(crate) fn get(core: &Core, params: &GetParams) -> Result<Value, Refusal> {
    let path = file_of(core, &params.package)?;
    let read = current(&params.package, &path)?;
    Ok(json!({ "values": read.values }))
}

/// `prefs.set`：照顶层的键并、`null` 删，写下、推给别的连接，交回并好的整份。和原来一样的不写、不推。
///
/// # Errors
///
/// 不是装着的界面包（[`file_of`]）；并好的超过 [`LIMIT`] 的 `prefs_too_big`；文件读不了、写不进的 `internal_error`。
pub(crate) fn set(core: &Core, caller: &Caller, params: &SetParams) -> Result<Value, Refusal> {
    let path = file_of(core, &params.package)?;
    let _held = core
        .prefs
        .lock
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    for _ in 0..TRIES {
        let read = current(&params.package, &path)?;
        let mut values = read.values.clone();
        merge(&mut values, &params.values);
        if values == read.values && read.bad.is_none() {
            return Ok(json!({ "values": values }));
        }
        let size = serde_json::to_vec(&values)
            .map_err(|_| Refusal::INTERNAL)?
            .len();
        if size > LIMIT {
            return Err(Refusal::prefs_too_big(size, LIMIT));
        }
        match file::write(&path, &values, read.version.as_deref()) {
            Ok(()) => {
                push(core, caller, &params.package, &values);
                return Ok(json!({ "values": values }));
            }
            Err(WriteError::Changed) => {}
            Err(error) => return Err(unwritten(&params.package, &error)),
        }
    }
    Err(unwritten(&params.package, &WriteError::Changed))
}

/// 订包 `package` 的设置（`subscribe` 的 `prefs` 流）：先过 [`file_of`]，先拿收推送的一头、再读一份，之后推的都不比读的旧。
/// 交回订阅的回应（这一刻的整份）和收推送的一头。
///
/// # Errors
///
/// 同 [`get`]。
pub(crate) fn follow(
    core: &Core,
    package: &str,
) -> Result<(Value, broadcast::Receiver<Arc<Changed>>), Refusal> {
    let path = file_of(core, package)?;
    let changes = core.prefs.changes.subscribe();
    let read = current(package, &path)?;
    Ok((json!({ "values": read.values }), changes))
}

/// 包 `package` 的设置文件在哪：装着的（`system_packages` 里有）、清单是 `[ui]` 的那个包的状态目录里。只开给界面包：扩展、小程序
/// 的状态目录归它们自己的程序，核心不往里放东西。
///
/// # Errors
///
/// 没装的、清单读不成的 `unknown_package`；别的种类的 `not_a_ui_package`。
fn file_of(core: &Core, package: &str) -> Result<PathBuf, Refusal> {
    let packages = core.system_packages();
    let found = packages
        .iter()
        .find(|found| found.id == package)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    match &found.read {
        Ok(manifest) if manifest.kind == PackageKind::Ui => {
            Ok(core.root.package_state(package).join(file::FILE))
        }
        Ok(_) => Err(Refusal::NOT_A_UI_PACKAGE),
        Err(_) => Err(Refusal::UNKNOWN_PACKAGE),
    }
}

/// 读这一刻的一份。不是一个对象的当空的，记一行。
///
/// # Errors
///
/// 读不了、太大、不是 UTF-8：`internal_error`，记一行。
fn current(package: &str, path: &Path) -> Result<file::Read, Refusal> {
    let read = file::read(path).map_err(|error| {
        tracing::warn!(target: "miyu::endpoint", package, error = %error, "prefs unreadable");
        Refusal::INTERNAL
    })?;
    if let Some(error) = &read.bad {
        tracing::warn!(target: "miyu::endpoint", package, error = error.as_str(), "prefs not an object");
    }
    Ok(read)
}

/// 照顶层的键并：`null` 的删掉，别的整个换上（对象不往里并）。
fn merge(values: &mut Map<String, Value>, changes: &Map<String, Value>) {
    for (key, value) in changes {
        match value {
            Value::Null => {
                values.remove(key);
            }
            value => {
                values.insert(key.clone(), value.clone());
            }
        }
    }
}

/// 广播改好的整份：没人订着的发不出去，不要紧。
fn push(core: &Core, caller: &Caller, package: &str, values: &Map<String, Value>) {
    let line = json!({"jsonrpc": "2.0", "method": "prefs.changed",
        "params": {"package": package, "values": values}});
    let changed = Changed {
        package: package.to_string(),
        from: caller.reverse.clone(),
        line: line.to_string(),
    };
    if core.prefs.changes.send(Arc::new(changed)).is_err() {
        // 没有订着的。
    }
}

/// 写不进：记一行，`internal_error`。
fn unwritten(package: &str, error: &WriteError) -> Refusal {
    tracing::warn!(target: "miyu::endpoint", package, error = %error, "prefs not written");
    Refusal::INTERNAL
}
