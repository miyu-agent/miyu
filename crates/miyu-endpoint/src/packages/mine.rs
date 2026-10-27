//! 装进、卸出自己那一层（施工 U23 下上，设计 06 第八又补第 5 条「自己装」，[`super::scope`]）：`home/<账号>/packages/`，只对他
//! 自己生效。只收不带程序的包（[`runs_program`]）；同编号的盖过系统那一份（系统那一份带程序的不盖，`package_exists`）。撞名、
//! 依赖照他看得到的那些比（[`Core::packages_seen`]）。
//!
//! 机器上的事都不动：不起停程序、不改系统配置、不碰系统那一层的本地库和状态目录；本地库记在他那层的 `.local` 里。装卸完清掉他
//! 那一份清单（[`super::own::Own::forget_account`]），只推给他自己的连接（[`super::changes::own_changed`]）。

use std::path::Path;
use std::sync::Arc;

use serde_json::{Map, Value, json};

use miyu_config::package::Manifest;
use miyu_kernel::id::AccountId;
use miyu_store::packages::conflicts::conflicts;
use miyu_store::packages::install::{self, Placed};
use miyu_store::packages::{Found, Issue, Layer, local, runs_program};

use super::folder::read;
use super::manage::{blocking, invalid};
use super::scope::{Scope, own_apart, own_dir};
use super::{TARGET, entry};
use crate::Core;
use crate::config::methods::words;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 从包文件夹 `path` 装进 `account` 自己那一层；`file` 是从哪个包文件解出来的（本地库记它）。
pub(super) async fn from_path(
    core: &Arc<Core>,
    peer: Peer,
    account: &AccountId,
    path: &Path,
    file: Option<&Path>,
) -> Result<Value, Refusal> {
    own_apart(core, account)?;
    let (folder, id, manifest) = admitted(core, peer, account, path)?;
    clear_of_conflicts(core, account, &id, manifest, folder.clone()).await?;
    let words = words(core, peer.language)?;
    let (dir, target, source) = (own_dir(core, account), id.clone(), folder.clone());
    let placed: Placed = blocking(move || install::place(&dir, &target, &source)).await?;
    core.own_packages.forget_account(account);
    let now = core.packages_seen(account);
    let mine = now
        .iter()
        .find(|one| one.id == id && one.layer == Layer::User);
    if let Some(Found {
        read: Err(Issue::Wrong(problem)),
        ..
    }) = mine
    {
        let refusal = invalid(&words, problem);
        if let Err(error) = placed.undo() {
            tracing::warn!(target: TARGET, package = id.as_str(), error = %error, "package install not undone");
        }
        core.own_packages.forget_account(account);
        return Err(refusal);
    }
    placed.keep();
    let version = mine
        .and_then(|one| one.read.as_ref().ok())
        .and_then(|manifest| manifest.version.clone());
    let (layer, root) = (
        own_dir(core, account),
        Scope::Own(account.clone()).local_root(core),
    );
    let (record_id, from) = (id.clone(), file.unwrap_or(&folder).display().to_string());
    if let Err(refusal) =
        blocking(move || super::local::record_at(&layer, &root, &record_id, version, &from)).await
    {
        tracing::warn!(target: TARGET, package = id.as_str(), reason = refusal.reason, "package not recorded");
    }
    super::changes::own_changed(core, account, &id);
    tracing::info!(target: TARGET, package = id.as_str(), "own package installed");
    entry(core, &id, peer, account).ok_or(Refusal::INTERNAL)
}

/// 装进 `account` 自己那一层之前查的（装、看一眼共用）：读得成（[`read`]）、不带程序（带的照 [`Scope::system_only`] 拒）、
/// 要盖的系统那一份不带程序；成员（不是管理员的）只能从他自己的地方装（[`reachable`]，调的一方查）。
pub(super) fn admitted(
    core: &Core,
    peer: Peer,
    account: &AccountId,
    path: &Path,
) -> Result<(std::path::PathBuf, String, Manifest), Refusal> {
    let (folder, id, manifest) = read(core, peer, path)?;
    if runs_program(&manifest) {
        Scope::Own(account.clone()).system_only(peer.admin)?;
    }
    let system = core.system_packages();
    if system
        .iter()
        .find(|one| one.id == id)
        .is_some_and(|one| one.read.as_ref().map_or(true, runs_program))
    {
        return Err(Refusal::PACKAGE_EXISTS);
    }
    Ok((folder, id, manifest))
}

/// 要装进 `account` 那一层的包（编号 `id`、清单 `manifest`、文件夹 `dir`）和他看得到的别的包、核心内置的斜杠命令重了名的
/// `package_conflict`，依赖的包他看不到的 `depends_missing`。人格、预设不和哪一层比：包带的照包认（同自己放的盖过系统的）。
pub(super) async fn clear_of_conflicts(
    core: &Arc<Core>,
    account: &AccountId,
    id: &str,
    manifest: Manifest,
    dir: std::path::PathBuf,
) -> Result<(), Refusal> {
    let seen = core.packages_seen(account);
    let others: Vec<&Found> = seen.iter().filter(|one| one.id != id).collect();
    let missing = super::depends::missing(&others, &manifest);
    if !missing.is_empty() {
        return Err(Refusal::depends_missing(&missing));
    }
    let reserved = core
        .commands
        .reserved(|package| super::is_installed(&seen, package));
    let id = id.to_string();
    let found = blocking(move || {
        Ok(conflicts(
            &id,
            &manifest,
            &dir,
            &seen,
            &reserved,
            &|_, _| Vec::new(),
        ))
    })
    .await?;
    match found.is_empty() {
        true => Ok(()),
        false => Err(Refusal::package_conflict(&found)),
    }
}

/// 卸 `account` 自己那一层的包 `id`；`preview` 的只看一眼。系统那一层、出厂的照 [`Scope::system_only`] 拒；被他看得到的别的包
/// 依赖着的卸不掉。
pub(super) async fn remove(
    core: &Arc<Core>,
    peer: Peer,
    account: &AccountId,
    id: String,
    preview: bool,
) -> Result<Value, Refusal> {
    if !miyu_store::personas::valid(&id) {
        return Err(Refusal::BAD_PARAMS);
    }
    own_apart(core, account)?;
    let seen = core.packages_seen(account);
    let Some(one) = seen
        .iter()
        .find(|one| one.id == id && one.layer == Layer::User)
    else {
        if seen.iter().any(|one| one.id == id) {
            Scope::Own(account.clone()).system_only(peer.admin)?;
        }
        return Err(Refusal::UNKNOWN_PACKAGE);
    };
    let by = super::depends::dependents(&seen, &id);
    if !by.is_empty() {
        return Err(Refusal::package_depended(&by));
    }
    let (dir, root) = (
        own_dir(core, account),
        Scope::Own(account.clone()).local_root(core),
    );
    if preview {
        return looking(one, &dir.join(&id)).await;
    }
    let target = id.clone();
    blocking(move || {
        install::take_out(&dir, &target)?;
        super::local::forget(&root, &target);
        Ok(())
    })
    .await?;
    core.own_packages.forget_account(account);
    super::changes::own_changed(core, account, &id);
    tracing::info!(target: TARGET, package = id.as_str(), "own package removed");
    Ok(json!({"package": id, "removed": true}))
}

/// 卸之前看一眼（同 `preview::removing` 的样子）：他那层没有配置项、状态目录。
async fn looking(one: &Found, folder: &Path) -> Result<Value, Refusal> {
    let mut answer = Map::new();
    answer.insert("package".into(), json!(one.id));
    answer.insert("layer".into(), json!(one.layer.as_str()));
    if let Some(version) = one.read.as_ref().ok().and_then(|it| it.version.clone()) {
        answer.insert("version".into(), json!(version));
    }
    let folder = folder.to_path_buf();
    let files = blocking(move || local::scan(&folder)).await?;
    answer.insert("files".into(), json!(files.len()));
    answer.insert("size".into(), json!(local::size(&files)));
    answer.insert("settings".into(), json!([]));
    answer.insert("state".into(), json!(false));
    Ok(Value::Object(answer))
}

/// 成员（不是管理员的，`admin` 是假的）从本机路径 `path` 装：只能是他自己家目录里的（读这台电脑的文件只给管理员，设计 06
/// 第八再又补第 2 条），别的 `not_permitted`。管理员不挡。碰磁盘。
pub(super) fn reachable(
    core: &Core,
    admin: bool,
    account: &AccountId,
    path: &Path,
) -> Result<(), Refusal> {
    if admin {
        return Ok(());
    }
    let home = std::fs::canonicalize(core.root.account_dir(account))
        .map_err(|_| Refusal::NOT_PERMITTED)?;
    match std::fs::canonicalize(path) {
        Ok(real) if real.starts_with(&home) => Ok(()),
        _ => Err(Refusal::NOT_PERMITTED),
    }
}
