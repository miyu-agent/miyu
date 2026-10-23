//! 管理员组（施工 Y-12 中下，设计 06 U16、U19 第 4 条）：管理员是机主，和组表 `system/groups.json` 里 `admin` 组的人；机主不写进表
//! 也算、撤不掉。管理员才能做的账号管理（发邀请、列账号、停用启用、设管理员）照 [`is_admin`] 判。`account.set_admin` 只给管理员：
//! 对机主、系统、包的账号、没有的账号回 `unknown_account`（同 `account.disable`），对停用的成员回 `member_disabled`。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_store::groups::ADMIN;

use super::invites::admin_only;
use super::{TARGET, files};
use crate::Core;
use crate::refusal::Refusal;

/// `account` 是不是管理员：机主，或者在管理员组里。
pub(crate) fn is_admin(core: &Core, account: &AccountId) -> bool {
    *account == core.admin || files::read_groups(core).groups.has(ADMIN, account.as_str())
}

/// `account.set_admin` 的参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetParams {
    account: String,
    admin: bool,
}

/// `account.set_admin {account, admin}`：管理员把一个成员设成、撤掉管理员，回 `{}`。本来就是、本来不是的照样回 `{}`。
pub(crate) async fn set(core: &Core, who: &AccountId, params: Value) -> Result<Value, Refusal> {
    admin_only(core, who)?;
    let params: SetParams = serde_json::from_value(params).map_err(|_| Refusal::BAD_PARAMS)?;
    let target = AccountId::parse(&params.account).map_err(|_| Refusal::UNKNOWN_ACCOUNT)?;
    if target == core.admin {
        return Err(Refusal::UNKNOWN_ACCOUNT);
    }
    let _queue = core.identity.files.lock().await;
    // 只有写在 `accounts.json` 里的成员能设：系统、包的账号不在那里（施工 Y-5 中下起名字和人互相占位）。
    let written = files::read_accounts(core).accounts;
    let row = written
        .of(target.as_str())
        .ok_or(Refusal::UNKNOWN_ACCOUNT)?;
    if row.disabled {
        return Err(Refusal::MEMBER_DISABLED);
    }
    let changed = files::change_groups(core, |groups| {
        Ok(match params.admin {
            true => groups.add(ADMIN, target.as_str()),
            false => groups.remove(ADMIN, target.as_str()),
        })
    })?;
    if changed {
        let now = crate::sessions::now();
        let kind = match params.admin {
            true => "account.admin_added",
            false => "account.admin_removed",
        };
        files::record(core, who, now, kind, &json!({"account": target.as_str()}));
        tracing::info!(target: TARGET, "admin group changed");
    }
    Ok(json!({}))
}
