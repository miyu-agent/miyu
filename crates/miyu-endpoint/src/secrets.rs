//! 密钥（`docs/blueprint/config.md`「协议」`secret.*`、「怎么走」第九条，G9，施工 8-5）：协议上的 `secret.set`、
//! `secret.delete`、`secret.list`，手改密钥文件被看到的，留痕。
//!
//! 只能写、删、列名字，从不交出值：值不进回应、推送、运行日志、报错的话、系统日志（`07-存储.md` 第九节）。`secret.set` 的
//! 参数不进运行日志：端点的 `DEBUG request` 那一行本来只记方法名。
//!
//! 1. `secret.set`：名字不合写法、值去掉前后空白是空的、有控制字符、超过 16 KiB：`bad_params`。拿着配置服务的锁，先把密钥
//!    文件重读一遍（手改过的先当手改记），读不进来的：`config_file_broken`。只改那一行（新建的先写一行开头的注释），照
//!    配置文件的规矩写盘、Unix 上 0600；替换前发现有人手改，从重读重来，最多三次。落了盘记 `secret.changed`、
//!    `INFO secret changed`，换上，再回应 `{"replaced"}`。
//! 2. `secret.delete`：同上；没有这个密钥：`unknown_secret`。回应 `{}`。
//! 3. `secret.list`：设了的，和系统配置、个人设置的最终值里引用了还没设的，照名字排；`used_by` 是引用它的项，照键名排。
//! 4. 手改被看到的（第九条第 3 条）：字节一样的什么都不做；变了的名字每个记一条 `secret.changed`（`via` 是 `file`，`by` 是
//!    内核，没有 `cause`）、`INFO secret changed`；读不进来的照上一次读好的用。不推：密钥的推送随界面（M9）。
//!
//! 写不成的：`internal_error`，记一条 `WARN config not written`，一个字节不动。
//!
//! 两份（施工 Y-2 下下中中，2026-10-24 项目主人定「成员只继承系统层」）：系统那一份 `system/secrets.toml` 大家共用，系统配置里
//! 引用的 key 写在这里；每个人自己的 `home/<账号>/secrets.toml` 只给他自己的配置用，按名字找时先找它（[`Config::secret`]）。
//! `secret.set`、`secret.delete` 多收 `layer`（`system`、`personal`），写系统那一份只有管理员；不写的，管理员模式里是系统
//! 那一份（施工 Y-12 下；没声明认识管理员模式的老头，管理员的照旧算在管理员模式里），平时、成员是他自己的；
//! `secret.list` 列这个连接的人用得上的两份，每个名字带 `layer`。核心自己替 MCP 服务存、清的登录令牌照旧在系统那一份。

mod file;

pub(crate) use file::SecretsFile;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use miyu_config::secret::{self, Secret};
use miyu_kernel::id::CommandId;
use miyu_kernel::origin::{By, Person};
use miyu_store::config_file::{self, WriteError};

use crate::Core;
use crate::config::journal;
use crate::config::methods::{said_at, words};
use crate::config::{Config, TARGET};
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 替换前发现有人手改，最多重来几次（第五条第 6 条）。
const TRIES: usize = 3;

/// 写哪一份（施工 Y-2 下下中中）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Shelf {
    /// 系统那一份，大家共用：只有管理员写得了。
    System,
    /// 这个人自己的。
    Personal,
}

impl Shelf {
    fn of(self, config: &Config) -> &SecretsFile {
        match self {
            Shelf::System => &config.secrets,
            Shelf::Personal => &config.own_secrets,
        }
    }

    fn of_mut(self, config: &mut Config) -> &mut SecretsFile {
        match self {
            Shelf::System => &mut config.secrets,
            Shelf::Personal => &mut config.own_secrets,
        }
    }
}

/// `secret.set` 的参数。值只在这里停一下，马上变成 [`Secret`]。
#[derive(Deserialize)]
pub(crate) struct SetParams {
    /// 密钥的名字。
    name: String,
    /// 密钥本身。
    value: String,
    /// 写哪一份（施工 Y-2 下下中中）：不写的照 [`whose`]。
    #[serde(default)]
    layer: Option<Shelf>,
}

/// `secret.delete` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct DeleteParams {
    /// 密钥的名字。
    name: String,
    /// 删哪一份：不写的照 [`whose`]。
    #[serde(default)]
    layer: Option<Shelf>,
}

/// 这个连接的人改谁的哪一份（施工 Y-2 下下中中）：系统那一份只给管理员；没写 `layer` 的，管理员模式里（`admin_mode`，施工
/// Y-12 下）是系统那一份，平时、成员是他自己的。系统那一份照机主那一份写：大家共用的只有一份，写好了照它发给别人那几份。
fn whose<'a>(
    core: &'a Core,
    caller: &crate::hello::Caller,
    layer: Option<Shelf>,
    admin_mode: bool,
) -> Result<(crate::config::people::Mine<'a>, Shelf), Refusal> {
    let account = caller.acting_for();
    let layer = layer.unwrap_or(if admin_mode {
        Shelf::System
    } else {
        Shelf::Personal
    });
    match layer {
        Shelf::System if !crate::login::admins::is_admin(core, account) => {
            Err(Refusal::NOT_PERMITTED)
        }
        Shelf::System => Ok((core.mine(&core.admin), layer)),
        Shelf::Personal => Ok((core.mine(account), layer)),
    }
}

/// `secret.changed` 的 `body`：名字、做了什么、怎么改的。只记名字，从不记值。
#[derive(Debug, Serialize)]
struct SecretChanged<'a> {
    /// 密钥的名字。
    name: &'a str,
    /// `set` 新设、`replaced` 换掉、`deleted` 删掉。
    action: &'static str,
    /// `set` 经 `secret.set`、`secret.delete`，`file` 手改被看到的。
    via: &'static str,
}

/// 谁改的：人经协议改的记在这个账号名下、带这条命令；核心自己清的（施工 X-42 上：删服务时清登录的令牌）记在内核名下。
#[derive(Clone, Copy)]
enum Who<'a> {
    /// 人，和这一条命令。
    Person(&'a CommandId),
    /// 核心自己。
    Core,
}

/// 改一个密钥：写入还是删掉。
enum Edit {
    /// 写入或者换掉。
    Set(Secret),
    /// 删掉。
    Delete,
}

/// `secret.set`：写入或者换掉一个密钥，落了盘回 `{"replaced"}`。`cause` 是这条命令的编号，记进日志。
pub(crate) fn set(
    core: &Core,
    (caller, peer): (&crate::hello::Caller, Peer),
    request: &crate::wire::Request,
    params: SetParams,
) -> Result<Value, Refusal> {
    if !secret::valid_name(&params.name) {
        return Err(Refusal::BAD_PARAMS);
    }
    let value = Secret::new(&params.value).map_err(|_| Refusal::BAD_PARAMS)?;
    let (mine, layer) = whose(core, caller, params.layer, peer.admin_mode(request))?;
    let cause = &request.id;
    let action = change(
        (core, &mine, layer),
        peer.language,
        Who::Person(cause),
        &params.name,
        Edit::Set(value),
    )?;
    Ok(json!({ "replaced": action == "replaced" }))
}

/// 核心自己替人存一个密钥（施工 X-42 上：网址的服务登录换来的令牌）：同 [`set`]，名字、值都是核心拼的，记在 `peer` 名下。
///
/// # Errors
///
/// 同 [`set`]。
pub(crate) fn store(
    core: &Core,
    peer: Peer,
    cause: &CommandId,
    name: &str,
    value: &str,
) -> Result<(), Refusal> {
    if !secret::valid_name(name) {
        return Err(Refusal::BAD_PARAMS);
    }
    let value = Secret::new(value).map_err(|_| Refusal::BAD_PARAMS)?;
    change(
        (core, &core.mine(&core.admin), Shelf::System),
        peer.language,
        Who::Person(cause),
        name,
        Edit::Set(value),
    )
    .map(|_| ())
}

/// 核心自己改一个密钥（施工 X-42 下：续期换来的令牌写回去）：同 [`store`]，记成核心改的。
///
/// # Errors
///
/// 同 [`set`]。
pub(crate) fn keep(core: &Core, name: &str, value: &str) -> Result<(), Refusal> {
    let value = Secret::new(value).map_err(|_| Refusal::BAD_PARAMS)?;
    let owner = core.mine(&core.admin);
    change(
        (core, &owner, Shelf::System),
        "en",
        Who::Core,
        name,
        Edit::Set(value),
    )
    .map(|_| ())
}

/// 替人删一个密钥（施工 X-42 下：`mcp.logout`）：同 [`delete`]，名字是核心拼的。没有的 `unknown_secret`。
///
/// # Errors
///
/// 同 [`delete`]。
pub(crate) fn remove(
    core: &Core,
    peer: Peer,
    cause: &CommandId,
    name: &str,
) -> Result<(), Refusal> {
    let owner = core.mine(&core.admin);
    change(
        (core, &owner, Shelf::System),
        peer.language,
        Who::Person(cause),
        name,
        Edit::Delete,
    )
    .map(|_| ())
}

/// 核心自己清掉一个密钥（施工 X-42 上：删服务时清登录的令牌）：没有的什么都不做；清不成的记一行。
pub(crate) fn discard(core: &Core, name: &str) {
    if !core.config().secrets.has(name) {
        return;
    }
    let owner = core.mine(&core.admin);
    if change(
        (core, &owner, Shelf::System),
        "en",
        Who::Core,
        name,
        Edit::Delete,
    )
    .is_err()
    {
        tracing::warn!(target: TARGET, name, "secret not discarded");
    }
}

/// `secret.delete`：删掉一个密钥，落了盘回 `{}`。没有这个密钥：`unknown_secret`。
pub(crate) fn delete(
    core: &Core,
    (caller, peer): (&crate::hello::Caller, Peer),
    request: &crate::wire::Request,
    params: DeleteParams,
) -> Result<Value, Refusal> {
    let (mine, layer) = whose(core, caller, params.layer, peer.admin_mode(request))?;
    let cause = &request.id;
    change(
        (core, &mine, layer),
        peer.language,
        Who::Person(cause),
        &params.name,
        Edit::Delete,
    )?;
    Ok(json!({}))
}

/// 改一个密钥：重读、改那一行、写盘，重来最多三次。交回做了什么（`set`、`replaced`、`deleted`）。
fn change(
    (core, mine, layer): (&Core, &crate::config::people::Mine<'_>, Shelf),
    language: &str,
    who: Who<'_>,
    name: &str,
    edit: Edit,
) -> Result<&'static str, Refusal> {
    let words = words(core, language)?;
    let Some(header) = miyu_config::Words::sentence(&words, "config/secrets-header", &[]) else {
        tracing::warn!(target: TARGET, error = "no words for config/secrets-header", "config words missing");
        return Err(Refusal::INTERNAL);
    };
    let mut config = mine.config();
    for _ in 0..TRIES {
        observe(core, &mut config, layer);
        let file = layer.of(&config);
        if file.broken.is_some() {
            let layers = config.layers(None);
            let mut problems = Vec::new();
            for problem in file.problems() {
                problems.push(said_at(
                    &config,
                    &layers,
                    problem,
                    Some((&file.shown, file.last_good)),
                    &words,
                )?);
            }
            return Err(Refusal::config_file_broken(problems));
        }
        let had = file.has(name);
        let (text, action) = match &edit {
            Edit::Set(value) => {
                let base = match file.version {
                    Some(_) => file.text.clone(),
                    None => format!("# {header}\n"),
                };
                let action = if had { "replaced" } else { "set" };
                (secret::set_in(&base, name, value), action)
            }
            Edit::Delete if !had => return Err(Refusal::UNKNOWN_SECRET),
            Edit::Delete => (secret::unset_in(&file.text, name), "deleted"),
        };
        let Ok(text) = text else {
            // 这个名字在文件里写成了别的样子（一张表）：这一行本来就报着错，先把文件改好。
            let layers = config.layers(None);
            let mut problems = Vec::new();
            for problem in file.problems() {
                problems.push(said_at(
                    &config,
                    &layers,
                    problem,
                    Some((&file.shown, file.last_good)),
                    &words,
                )?);
            }
            return Err(Refusal::config_file_broken(problems));
        };
        let bytes = config_file::bytes(&text, file.bom);
        match miyu_store::secrets::write(&file.path, &bytes, file.version.as_deref()) {
            Ok(()) => {
                let written = file.written(text, config_file::version(&bytes));
                *layer.of_mut(&mut config) = written;
                let (by, cause) = match who {
                    Who::Person(cause) => (
                        By::Person(Person::new(config.places.account.clone())),
                        Some(cause),
                    ),
                    Who::Core => (By::Kernel, None),
                };
                record(&config, name, action, "set", by, cause);
                crate::config::people::publish(core, &config, None);
                return Ok(action);
            }
            Err(WriteError::Changed) => {}
            Err(WriteError::Io(error)) => {
                tracing::warn!(target: TARGET, file = %file.shown, error = %error, "config not written");
                return Err(Refusal::INTERNAL);
            }
        }
    }
    tracing::warn!(target: TARGET, file = %layer.of(&config).shown, error = "changed while writing", "config not written");
    Err(Refusal::INTERNAL)
}

/// `secret.list`：这个连接的人用得上的两份里的名字、设没设、在哪一份（自己的盖着系统的）、谁在用，照名字排。
pub(crate) fn list(core: &Core, account: &miyu_kernel::id::AccountId) -> Value {
    let mine = core.mine(account);
    let config = mine.config();
    let mut listed: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for name in config
        .secrets
        .stored
        .entries
        .keys()
        .chain(config.own_secrets.stored.entries.keys())
    {
        listed.entry(name.clone()).or_default();
    }
    // 引用了它的真的键：密钥的列表（供应商的几个 key，施工 8-6）里的也算。
    for (name, key) in miyu_config::secret::used(&config.resolved().values()) {
        listed.entry(name).or_default().push(key);
    }
    let secrets: Vec<Value> = listed
        .into_iter()
        .map(|(name, mut used_by)| {
            used_by.sort_unstable();
            used_by.dedup();
            let layer = if config.own_secrets.has(&name) {
                json!("personal")
            } else if config.secrets.has(&name) {
                json!("system")
            } else {
                Value::Null
            };
            json!({"name": name, "set": !layer.is_null(), "layer": layer, "used_by": used_by})
        })
        .collect();
    json!({ "secrets": secrets })
}

/// 重读密钥文件（手改被看到的，`secret.*` 写之前）：一样的什么都不做；不一样的换上（读不进来的照上一次读好的用），
/// 变了的名字每个记一条，交给会话、核心（不推）。
pub(crate) fn observe(core: &Core, config: &mut Config, layer: Shelf) {
    let old = layer.of(config);
    let fresh = SecretsFile::read(&old.path, &old.shown).keeping(old);
    if fresh.same_as(old) {
        return;
    }
    told(&fresh);
    let changes = changes(old, &fresh);
    *layer.of_mut(config) = fresh;
    for (name, action) in &changes {
        record(config, name, action, "file", By::Kernel, None);
    }
    crate::config::people::publish(core, config, None);
}

/// 读到的密钥文件组、别人读得到：记一条 `WARN secrets readable by others`，照用，不去改它（第九条第 2 条）。
pub(crate) fn told(file: &SecretsFile) {
    if file.open {
        tracing::warn!(target: TARGET, file = %file.shown, "secrets readable by others");
    }
}

/// 两份之间变了的名字：新设、换掉、删掉，照名字排。
fn changes(old: &SecretsFile, new: &SecretsFile) -> Vec<(String, &'static str)> {
    let (before, after) = (&old.stored.entries, &new.stored.entries);
    let mut names: Vec<&String> = before.keys().chain(after.keys()).collect();
    names.sort_unstable();
    names.dedup();
    names
        .into_iter()
        .filter_map(|name| {
            let action = match (before.get(name), after.get(name)) {
                (None, Some(_)) => "set",
                (Some(was), Some(now)) if was != now => "replaced",
                (Some(_), None) => "deleted",
                _ => return None,
            };
            Some((name.clone(), action))
        })
        .collect()
}

/// 记一条 `secret.changed` 进系统日志，一条 `INFO secret changed`：只有名字。
fn record(
    config: &Config,
    name: &str,
    action: &'static str,
    via: &'static str,
    by: By,
    cause: Option<&CommandId>,
) {
    let body = SecretChanged { name, action, via };
    journal::record(
        &config.places.system_journal,
        &format!("system/{}", miyu_store::journal::FILE),
        crate::sessions::now(),
        by,
        cause,
        "secret.changed",
        &body,
    );
    tracing::info!(target: TARGET, name, action, via, "secret changed");
}

#[cfg(test)]
mod tests;
