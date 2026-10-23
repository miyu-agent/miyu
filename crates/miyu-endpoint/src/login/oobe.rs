//! OOBE 时注册机主（施工 Y-12 中上，设计 06 U22，2026-10-23 项目主人定「OOBE 的时候引导注册」）：`admin` 是系统、不登录，
//! 装 Miyu 的那个人在 OOBE 时注册自己的账号，标成机主（`accounts.json` 的 `owner`），本机令牌从此就是他。
//!
//! 还没有机主的核心只跑「设置那一段」（[`Core::in_setup`]）：本机令牌的连接只能握手、设置（`account.setup`，或者先要一次性码给
//! 远程的网页设置）。注册成了：这个连接答完就断，别的连接一起断（[`Revoked::Everyone`]），交回的 `Notify` 响一下；起核心的一方
//! 收掉这一份，在同一个进程、同一个套接字上照机主起完整的那一份，头重连进来就是他。

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Notify;

use miyu_kernel::id::AccountId;
use miyu_store::accounts::Account;

use super::{Revoked, TARGET, Via, announce, files, members};
use crate::Core;
use crate::refusal::Refusal;

impl Core {
    /// 同一份，还没有机主：只能设置（施工 Y-12 中上）。注册了机主时 [`Core::setup_done`] 响一下。
    #[must_use]
    pub fn in_setup(mut self) -> Core {
        self.identity.setup = Some(Arc::new(Notify::new()));
        self
    }

    /// 设置那一段：注册了机主时响一下（`notify_one`，晚来等的照样收得到）。不在设置那一段的没有。
    pub fn setup_done(&self) -> Option<Arc<Notify>> {
        self.identity.setup.clone()
    }

    /// 在设置那一段：还没有机主。
    pub(crate) fn setting_up(&self) -> bool {
        self.identity.setup.is_some()
    }
}

/// 设置那一段的 `account.setup`：用户名 `username` 注册成机主，密码的整串是 `hashed`。用户名同注册（`members::taken`：`admin`、
/// 系统账号、已有的都占着）；已经有机主了的（两个头同时设置，后到的）`not_permitted`。交回回应和这个连接以后怎么算。
pub(super) async fn owner(
    core: &Arc<Core>,
    username: &str,
    hashed: String,
) -> Result<(Value, Via), Refusal> {
    let account = AccountId::parse(username).map_err(|_| Refusal::BAD_PARAMS)?;
    let _queue = core.identity.files.lock().await;
    if files::read_accounts(core).accounts.owner().is_some() {
        return Err(Refusal::NOT_PERMITTED);
    }
    if members::taken(core, &account) {
        return Err(Refusal::USERNAME_TAKEN);
    }
    let now = crate::sessions::now();
    core.root.prepare_home(&account).map_err(|error| {
        tracing::warn!(target: TARGET, kind = ?error.kind(), "home not prepared");
        Refusal::INTERNAL
    })?;
    files::change_accounts(core, |accounts| {
        accounts.put(Account {
            id: account.as_str().to_string(),
            username: account.as_str().to_string(),
            password: hashed,
            changed: now,
            created: Some(now),
            disabled: false,
            owner: true,
        });
        Ok(())
    })?;
    let (token, hash, expires) = files::issue(core, &account, now)?;
    tracing::info!(target: TARGET, "owner registered");
    // 设置那一段的连接都断开，交回的那一响让起核心的一方照机主重起（这个连接答完自己断）。
    announce(&core.identity, Revoked::Everyone);
    if let Some(done) = core.setup_done() {
        done.notify_one();
    }
    let reply = json!({
        "username": account.as_str(),
        "account": account.as_str(),
        "login": {"expires": expires, "token": token},
    });
    Ok((reply, Via::Login(hash, account)))
}
