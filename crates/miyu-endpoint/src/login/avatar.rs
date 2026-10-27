//! 账号头像（施工 AV-1，接入通讯平台设计 07 第 4.6 节第 8 样）：登录页「选择账号」画一排头像。一张 PNG、JPEG、WebP，放在
//! 账号的家目录 `home/<账号>/avatar.<扩展名>`，上限同人格的头像（1 MiB、1024 像素），查法、写法同它（`personas/picture.rs`）。
//!
//! - `account.set_avatar {"blob"} | {"unset": true}`，可带 `expect`：换、删自己的，回 `{"avatar": <版本或 null>}`。`expect` 对不上的
//!   `avatar_conflict`，带现在的版本。
//! - `account.avatar {"account"?}`：读，回 `{"avatar", "media_type", "data"}`，没有的是 `null`。自己的谁都读得了，别人的只给管理员。
//! - 握手回应、`account.list` 带版本 `avatar`（有的才带）：头照版本换缓存。这台浏览器登录过哪些账号由网页自己记，核心不列给
//!   没登录的人（项目主人 10-27 定）。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_store::personas::avatar_in;

use crate::Core;
use crate::personas::picture::{self, Picture, PictureParams};
use crate::refusal::Refusal;

/// 账号 `account` 的头像版本；没有的没有。
pub(crate) fn version(core: &Core, account: &AccountId) -> Option<String> {
    avatar_in(&core.root.account_dir(account))
        .as_deref()
        .and_then(picture::version_of)
}

/// `account.set_avatar`：见模块说明。
pub(crate) async fn set(
    core: &Core,
    account: &AccountId,
    params: PictureParams,
) -> Result<Value, Refusal> {
    let checked = picture::check(core, account, Picture::Avatar, params).await?;
    let dir = core.root.account_dir(account);
    if let Some(expect) = checked.expect() {
        let current = version(core, account);
        if &current != expect {
            return Err(Refusal::conflict("avatar_conflict", Some(json!(current))));
        }
    }
    picture::replace(&dir, &checked).map_err(|error| {
        tracing::warn!(target: "miyu::endpoint", kind = ?error.kind(), "account avatar not written");
        Refusal::INTERNAL
    })?;
    Ok(json!({"avatar": version(core, account)}))
}

/// `account.avatar` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadParams {
    #[serde(default)]
    account: Option<String>,
}

/// `account.avatar`：见模块说明。别人的、不是管理员的 `not_permitted`；账号写法不对的 `bad_params`。
pub(crate) async fn read(
    core: &Core,
    mine: &AccountId,
    params: ReadParams,
) -> Result<Value, Refusal> {
    let account = match params.account {
        Some(name) => AccountId::parse(&name).map_err(|_| Refusal::bad_field("account"))?,
        None => mine.clone(),
    };
    if account != *mine && !super::admins::is_admin(core, mine) {
        return Err(Refusal::NOT_PERMITTED);
    }
    let dir = core.root.account_dir(&account);
    tokio::task::spawn_blocking(move || match avatar_in(&dir) {
        Some(path) => picture::encoded(&path, Picture::Avatar).map_err(|_| Refusal::INTERNAL),
        None => Ok(Value::Null),
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?
}
