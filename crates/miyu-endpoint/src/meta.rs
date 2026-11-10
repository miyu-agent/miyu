//! 改标题、置顶、项目的参数（施工 3-8 三补、H-2，`docs/blueprint/protocol.md` 的 `session.set_meta`）：标题去掉前后空白，1 到
//! 200 个字；写 `null` 是去掉标题，记成空的标题（`kernel/events-bodies.md` 的 `session.meta_changed`）；项目写 `null` 是移出来，
//! 同样记成空的；三格都不写的是参数不对。
//!
//! 人要求重起标题的 `session.retitle`（施工 TT-1）也在这里：不带标题，交给内核照自动起标题的那一条路再起一次。

use std::sync::Arc;

use serde::{Deserialize, Deserializer};

use miyu_kernel::id::SessionId;
use miyu_kernel::session::{Command, Reason};

use crate::Core;
use crate::refusal::Refusal;
use crate::wire::Request;

/// 标题最多几个字（Unicode 字符），去掉前后空白以后数。
const TITLE_CHARS: usize = 200;

/// `session.set_meta` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct MetaParams {
    /// 哪个会话。
    pub(crate) session: String,
    /// 新的标题：没写是不改，写 `null` 是去掉，别的照写的。和别的「可以不写」的格不一样，`null` 有意思，所以记着写没写。
    #[serde(default, deserialize_with = "written")]
    title: Option<Option<String>>,
    /// 置顶还是取消置顶；写 `null` 和没写一样。
    #[serde(default)]
    pinned: Option<bool>,
    /// 移进哪个项目（施工 H-2）：没写是不改，写 `null` 是移出来。
    #[serde(default, deserialize_with = "written")]
    project: Option<Option<String>>,
}

impl MetaParams {
    /// 换成交给会话的命令：标题去掉前后空白、量过长短，去掉标题的写成空的。两格都不写（`pinned` 写 `null` 也算没写）、标题
    /// 去掉空白以后是空的、超过 200 个字的，是参数不对。
    pub(crate) fn command(&self) -> Result<Command, Refusal> {
        if self.title.is_none() && self.pinned.is_none() && self.project.is_none() {
            return Err(Refusal::BAD_PARAMS);
        }
        if self.project().is_some_and(str::is_empty) {
            return Err(Refusal::bad_field("project"));
        }
        let title = match &self.title {
            None => None,
            Some(None) => Some(String::new()),
            Some(Some(title)) => Some(trimmed(title)?),
        };
        Ok(Command::SetMeta {
            title,
            pinned: self.pinned,
            project: self
                .project
                .as_ref()
                .map(|project| project.clone().unwrap_or_default()),
        })
    }

    /// 要移进的项目：写了编号的才有（移出来、没写的没有）。端点照它查项目在不在、这个会话能不能进项目。
    pub(crate) fn project(&self) -> Option<&str> {
        self.project.as_ref()?.as_deref()
    }

    /// 写了 `project` 这一格（移进、移出都算）。
    pub(crate) fn moves(&self) -> bool {
        self.project.is_some()
    }
}

/// 写了这一格：`null` 读成 `Some(None)`，没写的由 `default` 读成 `None`。
fn written<'de, D: Deserializer<'de>>(read: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(read).map(Some)
}

/// 去掉前后空白的标题：空的、太长的是参数不对。
fn trimmed(title: &str) -> Result<String, Refusal> {
    let title = title.trim();
    match title.chars().count() {
        1..=TITLE_CHARS => Ok(title.to_string()),
        _ => Err(Refusal::BAD_PARAMS),
    }
}

/// `session.retitle` 的参数（施工 TT-1）。
#[derive(Debug, Deserialize)]
struct RetitleParams {
    /// 哪个会话。
    session: String,
}

/// `session.retitle`：照现在的对话重起一次标题（施工 TT-1，`docs/blueprint/protocol.md` 的 `session.retitle`）。请求发出去
/// 就回 `{}`，不等模型：起好了的标题经 `session.meta_changed` 推来，`by` 是内核，不算人起的。
///
/// # Errors
///
/// 参数读不成、会话编号不合写法、没东西可起（她一个带正文的回复都没有、这时也没有回合在进行）的是参数不对；没有这个会话、
/// 会话停了的照 `session.set_meta`。
pub(crate) async fn retitle(
    core: &Arc<Core>,
    request: &Request,
) -> Result<serde_json::Value, Refusal> {
    let params: RetitleParams = crate::methods::params(request)?;
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let found = core.sessions.get(core, &session).await?;
    crate::methods::command_to(core, request, &session, &found.handle, Command::Retitle)
        .await
        .map_err(
            |refusal| match refusal.reason == Reason::NothingToTitle.code() {
                true => Refusal::BAD_PARAMS,
                false => refusal,
            },
        )?;
    Ok(serde_json::json!({}))
}
