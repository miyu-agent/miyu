//! 邀请码（施工 Y-1 上，设计 06 第六节、U15）：管理员发、列、撤；握手时认（不用掉，注册成了才用掉，[`super::members`]）。
//!
//! 码是 16 个随机字节写成 32 位十六进制，原文只在 `account.invite` 的回应里出现一次；`invites.json` 只存它的 SHA-256。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_kernel::time::Timestamp;
use miyu_store::invites::Invite;
use miyu_store::logins::digest;

use super::{TARGET, Via, files, random_hex, stamp};
use crate::Core;
use crate::refusal::Refusal;

/// 不写 `days` 时有效几天（设计 06 第六节的初值）。
const DAYS: u64 = 7;

/// 最多有效几天。
const MOST_DAYS: u64 = 30;

/// 只给管理员：连接的账号 `account` 不是管理员的拒。
pub(super) fn admin_only(core: &Core, account: &AccountId) -> Result<(), Refusal> {
    match *account == core.admin {
        true => Ok(()),
        false => Err(Refusal::NOT_PERMITTED),
    }
}

/// `account.invite` 的参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InviteParams {
    #[serde(default)]
    days: Option<u64>,
}

/// `account.invite`：管理员发一个邀请码，交回码（只这一次）、编号、过期的时刻。
pub(crate) async fn invite(
    core: &Core,
    account: &AccountId,
    params: Value,
) -> Result<Value, Refusal> {
    admin_only(core, account)?;
    let params: InviteParams = serde_json::from_value(params).map_err(|_| Refusal::BAD_PARAMS)?;
    let days = params.days.unwrap_or(DAYS);
    if !(1..=MOST_DAYS).contains(&days) {
        return Err(Refusal::BAD_PARAMS);
    }
    let code: String = random_hex()?.chars().take(32).collect();
    let id: String = random_hex()?.chars().take(8).collect();
    let now = crate::sessions::now();
    let expires = Timestamp::from_unix_millis(
        now.unix_millis() + i64::try_from(days).unwrap_or(1) * 86_400_000,
    )
    .unwrap_or(now);
    let invite = Invite {
        id: id.clone(),
        hash: digest(&code),
        by: account.as_str().to_string(),
        created: now,
        expires,
        used: None,
    };
    let _queue = core.identity.files.lock().await;
    files::change_invites(core, |invites| {
        invites.add(invite, now);
        Ok(())
    })?;
    tracing::info!(target: TARGET, invite = id.as_str(), days, "invite issued");
    files::record(
        core,
        account,
        now,
        "account.invited",
        &json!({"invite": id, "expires": expires}),
    );
    Ok(json!({"code": code, "id": id, "expires": stamp(expires.unix_millis())}))
}

/// `account.invites`：管理员列出还在的邀请码（没过期没用的、最近用掉的），不带码。
pub(crate) fn list(core: &Core, account: &AccountId) -> Result<Value, Refusal> {
    admin_only(core, account)?;
    let mut invites = files::read_invites(core).invites;
    invites.prune(crate::sessions::now());
    let listed: Vec<Value> = invites
        .invites
        .iter()
        .map(|each| {
            let mut one = json!({
                "id": each.id,
                "created": each.created,
                "expires": each.expires,
            });
            if let Some(used) = &each.used {
                one["used"] = json!({"account": used.account, "at": used.at});
            }
            one
        })
        .collect();
    Ok(json!({"invites": listed}))
}

/// `account.invite_revoke` 的参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RevokeParams {
    id: String,
}

/// `account.invite_revoke`：管理员撤掉一个还没用掉的；没有、已经用了的 `unknown_invite`。
pub(crate) async fn revoke(
    core: &Core,
    account: &AccountId,
    params: Value,
) -> Result<Value, Refusal> {
    admin_only(core, account)?;
    let params: RevokeParams = serde_json::from_value(params).map_err(|_| Refusal::BAD_PARAMS)?;
    let now = crate::sessions::now();
    let _queue = core.identity.files.lock().await;
    files::change_invites(core, |invites| match invites.revoke(&params.id) {
        true => Ok(()),
        false => Err(Refusal::UNKNOWN_INVITE),
    })?;
    tracing::info!(target: TARGET, invite = params.id.as_str(), "invite revoked");
    files::record(
        core,
        account,
        now,
        "account.invite_revoked",
        &json!({"invite": params.id}),
    );
    Ok(json!({"revoked": true}))
}

/// 握手时的邀请码 `code`：能用的交回这个连接怎么算（只能注册），不用掉。
pub(crate) fn admit(core: &Core, code: &str) -> Option<Via> {
    let hash = digest(code);
    files::read_invites(core)
        .invites
        .usable(&hash, crate::sessions::now())
        .map(|_| Via::Invite(hash))
}
