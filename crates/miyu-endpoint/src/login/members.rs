//! 成员（施工 Y-1 上，设计 06 第二节、第六节）：凭邀请码注册、管理员列出、停用、启用。
//!
//! 用户名就是账号编号（家目录的名字），照账号编号的写法；管理员（`admin`）、管理员登录用的用户名、已有的账号、装着的包和本机
//! 那份官方索引里的包的编号（施工 Y-5 中下：带进程的包各是自己的账号）、组名 `users` 都不行。注册时建家目录和工作区、写 `accounts.json`、用掉邀请码、造登录令牌，都在改凭据文件的那把
//! 锁里一件件来；用户名占了的不用掉邀请码。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_store::accounts::{self, Account};

use super::invites::admin_only;
use super::{Revoked, TARGET, Via, announce, files, good_password};
use crate::Core;
use crate::refusal::Refusal;

/// 组名：所有账号默认在这个组里（设计 06 U11），不能拿来当用户名。
const USERS: &str = "users";

/// 系统的账号名（施工 Y-12 中上，设计 06 U22）：`admin` 是系统、不登录，谁都注册不成它。
const SYSTEM: &str = "admin";

/// 系统日志里注册成了的那一条。
const CREATED: &str = "account.created";

/// `account.register` 的参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterParams {
    username: String,
    password: String,
}

/// `account.register`：用邀请码握手的连接注册。交回回应和这个连接以后怎么算（就是这个新账号）。
pub(crate) async fn register(
    core: &Core,
    via: &Via,
    params: Value,
) -> Result<(Value, Via), Refusal> {
    let Via::Invite(invite) = via else {
        return Err(Refusal::BAD_PARAMS);
    };
    let params: RegisterParams = serde_json::from_value(params).map_err(|_| Refusal::BAD_PARAMS)?;
    let account = AccountId::parse(&params.username).map_err(|_| Refusal::BAD_PARAMS)?;
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
    if taken(core, &account) {
        return Err(Refusal::USERNAME_TAKEN);
    }
    let now = crate::sessions::now();
    let used = files::change_invites(core, |invites| {
        invites
            .take(invite, account.as_str(), now)
            .ok_or(Refusal::BAD_INVITE)
    })?;
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
            owner: false,
        });
        Ok(())
    })?;
    let (token, hash, expires) = files::issue(core, &account, now)?;
    tracing::info!(target: TARGET, invite = used.as_str(), "account registered");
    files::record(
        core,
        &account,
        now,
        CREATED,
        &json!({"account": account.as_str(), "invite": used}),
    );
    let reply = json!({
        "account": account.as_str(),
        "username": account.as_str(),
        "login": {"expires": expires, "token": token},
    });
    Ok((reply, Via::Login(hash, account)))
}

/// 用户名 `account` 能不能注册：人占着的（[`person_holds`]）、家目录已经在的、装着的包的编号、本机那份官方索引里的包的编号
/// （施工 Y-5 中下，两头互相占位）都占着。
pub(super) fn taken(core: &Core, account: &AccountId) -> bool {
    let name = account.as_str();
    person_holds(core, name)
        || core.root.account_dir(account).exists()
        || core.packages().iter().any(|one| one.id == name)
        || crate::packages::source::sync::indexed_ids(core)
            .iter()
            .any(|id| id == name)
}

/// 名字 `name` 是不是人这边占着的：机主、`admin`（系统，施工 Y-12 中上）、`users`、`accounts.json` 里的账号和登录用的用户名。
/// 装包时照它挡（施工 Y-5 中下）：包的编号就是它的账号名，不能和人撞。
pub(crate) fn person_holds(core: &Core, name: &str) -> bool {
    let written = files::read_accounts(core).accounts;
    core.admin.as_str() == name
        || name == SYSTEM
        || name == USERS
        || written.find(name).is_some()
        || written.of(name).is_some()
}

/// 登记过的人（`accounts.json` 里的，停用了的也在）：用量照他们补（施工 Y-2 下下中下）。
pub(crate) fn people(core: &Core) -> Vec<AccountId> {
    files::read_accounts(core)
        .accounts
        .accounts
        .iter()
        .filter_map(|row| AccountId::parse(&row.id).ok())
        .collect()
}

/// `account.list`：管理员列出账号：管理员、成员、系统账号。
pub(crate) fn list(core: &Core, account: &AccountId) -> Result<Value, Refusal> {
    admin_only(core, account)?;
    let written = files::read_accounts(core).accounts;
    let mut listed = vec![{
        let mut admin = json!({"account": core.admin.as_str(), "kind": "admin", "admin": true});
        if let Some(row) = written.of(core.admin.as_str()) {
            admin["username"] = json!(row.username);
        }
        admin
    }];
    let admins = files::read_groups(core).groups;
    for row in written
        .accounts
        .iter()
        .filter(|row| row.id != core.admin.as_str())
    {
        let admin = admins.has(miyu_store::groups::ADMIN, &row.id);
        let mut one =
            json!({"account": row.id, "username": row.username, "kind": "member", "admin": admin});
        if let Some(created) = row.created {
            one["created"] = json!(created);
        }
        if row.disabled {
            one["disabled"] = json!(true);
        }
        listed.push(one);
    }
    for system in core.system_accounts() {
        listed.push(json!({"account": system.as_str(), "kind": "system", "admin": false}));
    }
    Ok(json!({"accounts": listed}))
}

/// `account.disable`、`account.enable` 的参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SwitchParams {
    account: String,
}

/// `account.disable`（`disabled` 是真）、`account.enable`：管理员停用、启用一个成员。停用当场作废它全部登录令牌、断开它的
/// 连接，数据留着。管理员、系统账号、没有的账号 `unknown_account`。
pub(crate) async fn switch(
    core: &Core,
    who: &AccountId,
    params: Value,
    disabled: bool,
) -> Result<Value, Refusal> {
    admin_only(core, who)?;
    let params: SwitchParams = serde_json::from_value(params).map_err(|_| Refusal::BAD_PARAMS)?;
    let target = AccountId::parse(&params.account).map_err(|_| Refusal::UNKNOWN_ACCOUNT)?;
    if target == core.admin {
        return Err(Refusal::UNKNOWN_ACCOUNT);
    }
    let _queue = core.identity.files.lock().await;
    files::change_accounts(core, |accounts| {
        let row = accounts
            .accounts
            .iter_mut()
            .find(|row| row.id == target.as_str())
            .ok_or(Refusal::UNKNOWN_ACCOUNT)?;
        row.disabled = disabled;
        Ok(())
    })?;
    let now = crate::sessions::now();
    if disabled {
        let revoked = files::revoke(core, &target, None)?;
        announce(&core.identity, Revoked::All(target.clone()));
        tracing::info!(target: TARGET, count = revoked, "account disabled");
    } else {
        tracing::info!(target: TARGET, "account enabled");
    }
    let kind = match disabled {
        true => "account.disabled",
        false => "account.enabled",
    };
    files::record(core, who, now, kind, &json!({"account": target.as_str()}));
    Ok(json!({"account": target.as_str(), "disabled": disabled}))
}
