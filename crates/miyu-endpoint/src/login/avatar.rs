//! 你的头像（施工 AV-1；施工 SU-1 起只有主人一张）：一张 PNG、JPEG、WebP，放在主人的家目录 `avatar.<扩展名>`，上限同人格的头像
//! （1 MiB、1024 像素），查法、写法同它（`personas/picture.rs`）。
//!
//! - `account.set_avatar {"blob"} | {"unset": true}`，可带 `expect`：换、删，回 `{"avatar": <版本或 null>}`。`expect` 对不上的
//!   `avatar_conflict`，带现在的版本。
//! - `account.avatar {}`：读，回 `{"avatar", "media_type", "data"}`，没有的是 `null`。
//! - 握手回应带版本 `avatar`（有的才带）：头照版本换缓存。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_store::personas::avatar_in;

use crate::Core;
use crate::personas::picture::{self, Picture, PictureParams};
use crate::refusal::Refusal;

/// 主人的头像版本；没有的没有。
pub(crate) fn version(core: &Core) -> Option<String> {
    avatar_in(core.root.path())
        .as_deref()
        .and_then(picture::version_of)
}

/// `account.set_avatar`：见模块说明。
pub(crate) async fn set(core: &Core, params: PictureParams) -> Result<Value, Refusal> {
    let checked = picture::check(core, Picture::Avatar, params).await?;
    let dir = core.root.path().to_path_buf();
    if let Some(expect) = checked.expect() {
        let current = version(core);
        if &current != expect {
            return Err(Refusal::conflict("avatar_conflict", Some(json!(current))));
        }
    }
    picture::replace(&dir, &checked).map_err(|error| {
        tracing::warn!(target: "miyu::endpoint", kind = ?error.kind(), "account avatar not written");
        Refusal::INTERNAL
    })?;
    Ok(json!({"avatar": version(core)}))
}

/// `account.avatar` 的参数：没有格（施工 SU-1 起只有主人一张）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadParams {}

/// `account.avatar`：见模块说明。
pub(crate) async fn read(core: &Core, _params: ReadParams) -> Result<Value, Refusal> {
    let dir = core.root.path().to_path_buf();
    tokio::task::spawn_blocking(move || match avatar_in(&dir) {
        Some(path) => picture::encoded(&path, Picture::Avatar).map_err(|_| Refusal::INTERNAL),
        None => Ok(Value::Null),
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?
}
