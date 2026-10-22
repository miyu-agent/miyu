//! 换会话在哪个目录干活（施工 9-7 上，`docs/blueprint/protocol.md`「`session.set_workspace`」）：工作区是会话的属性，只有人
//! 明确换才变（网页里选、`/workspace`），一个头换了别的头跟着变（订阅着的收到 `session.workspace_changed`，会话列表那一项的
//! `cwd` 跟着变）。头每句话报的目录不再换它。
//!
//! 人明着换，写错了当场说，不悄悄退回：换不成真实位置、读不了的 `path_unreadable`，是文件的 `not_a_directory`，落在数据根里
//! 又不是账号自己的工作区的 `path_forbidden`。太宽的（系统的家目录、根目录、包含数据根的）照人选的用，回应多 `wide`（施工
//! 9-7 补，2026-10-09 项目主人定，原来退回账号的工作区）。加进来的目录照造会话的规矩查（`dir_too_wide`）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::session::Command;
use miyu_session::Handle;

use crate::Core;
use crate::methods::command_to;
use crate::refusal::Refusal;
use crate::sessions::{Found, Picked, check_dirs, roots, to_default, wide};
use crate::wire::Request;

/// `session.set_workspace` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetParams {
    /// 会话编号。
    session: String,
    /// 新的工作目录：绝对路径或者 `~` 开头的，人看到的那种写法；不写的照旧（只换加进来的目录）。
    #[serde(default)]
    cwd: Option<String>,
    /// 新的加进来的目录：写了的整份换掉，空的是去掉全部；不写的照旧。
    #[serde(default)]
    dirs: Option<Vec<String>>,
}

/// `session.set_workspace`：查过路径，交给会话换（和现在一样的什么都不记），会话表记着的跟着换。回应 `{"cwd", "dirs"}`：实际
/// 用的工作目录、现在加进来的目录。
pub(crate) async fn set(
    core: &Arc<Core>,
    request: &Request,
    params: SetParams,
) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    if params.cwd.is_none() && params.dirs.is_none() {
        return Err(Refusal::BAD_PARAMS);
    }
    if let Some(dirs) = &params.dirs {
        check_dirs(core, dirs)?;
    }
    let found = core.sessions.get(core, &session).await?;
    // 落在数据根里的照会话的属主认它自己的工作区（施工 O-4 下）。
    let picked = params
        .cwd
        .as_deref()
        .map(|cwd| checked(core, &found.handle, cwd))
        .transpose()?;
    let wide = picked.as_ref().is_some_and(|picked| picked.wide);
    // 没写工作目录的：照会话现在的。
    let cwd = picked.map_or_else(|| found.cwd.clone(), |picked| picked.cwd);
    let command = Command::SetWorkspace {
        cwd: cwd.clone(),
        dirs: params.dirs.clone(),
    };
    command_to(core, request, &session, &found.handle, command).await?;
    let dirs = core
        .sessions
        .moved(&session, cwd.clone(), params.dirs)
        .await
        .unwrap_or_default();
    let mut reply = json!({"cwd": cwd, "dirs": dirs});
    if wide {
        reply["wide"] = json!(true);
    }
    Ok(reply)
}

/// `session.send` 带的 `cwd`（施工 9-7 再补，终端无缝集成：每一轮的工作目录跟着 shell 走，设计 22 O6）：照 [`checked`] 查，和
/// 会话现在的一样什么都不做、不记；不一样的交给会话换（推 `session.workspace_changed`），会话表跟着换，`found` 的跟着改。交回
/// 太宽没有。和 9-7 上拿掉的头每句话报的目录不是一回事：那是头自己的目录，这是人在 shell 里 `cd` 过去的。
pub(crate) async fn follow(
    core: &Arc<Core>,
    request: &Request,
    session: &SessionId,
    found: &mut Found,
    cwd: &str,
) -> Result<bool, Refusal> {
    let picked = checked(core, &found.handle, cwd)?;
    if picked.cwd == found.cwd {
        return Ok(false);
    }
    let command = Command::SetWorkspace {
        cwd: picked.cwd.clone(),
        dirs: None,
    };
    // 换目录是这一句之外的另一条命令：编号照这一句的接 `/cwd`，和这一句不撞，重发的照样认得出（命令编号最长 128 个字节，
    // 接不下的不收）。
    let moving = Request {
        id: CommandId::parse(&format!("{}/cwd", request.id.as_str()))
            .map_err(|_| Refusal::BAD_PARAMS)?,
        method: request.method.clone(),
        params: Value::Null,
    };
    command_to(core, &moving, session, &found.handle, command).await?;
    core.sessions.moved(session, picked.cwd.clone(), None).await;
    found.cwd = picked.cwd;
    Ok(picked.wide)
}

/// 核心所在的机器上人的家目录，真实的位置；没有的、换不成的没有。
pub(crate) fn home(core: &Core) -> Option<PathBuf> {
    core.home
        .as_deref()
        .and_then(|home| std::fs::canonicalize(home).ok())
}

/// 查一个人明着要换去的工作目录，交回实际用的：见模块的说明。`/workspace` 也照它查（施工 9-7 下）。账号自己的工作区照会话的
/// 属主（施工 O-4 下）。人明着选的，太宽的照用、标上 `wide`（施工 9-7 补）。账号的工作区本身换成会话 `handle` 自己的默认
/// 工作区（施工 5-14 中：网页的「默认工作区」交的是它）。
pub(crate) fn checked(core: &Core, handle: &Handle, cwd: &str) -> Result<Picked, Refusal> {
    let owner = handle.owner();
    if let Some(picked) = to_default(core, owner, cwd, handle.default_workspace()) {
        return Ok(picked);
    }
    let home = home(core);
    let real = miyu_fs::resolve(Path::new("/"), home.as_deref(), cwd)
        .map_err(|_| Refusal::PATH_UNREADABLE)?;
    let meta = std::fs::metadata(&real).map_err(|_| Refusal::PATH_UNREADABLE)?;
    if !meta.is_dir() {
        return Err(Refusal::NOT_A_DIRECTORY);
    }
    let (data_root, own) = roots(core, owner);
    if real.starts_with(&data_root) && !real.starts_with(&own) {
        return Err(Refusal::PATH_FORBIDDEN);
    }
    Ok(Picked {
        cwd: cwd.to_string(),
        wide: wide(core, owner, &real),
        broad: miyu_fs::broad(&real, home.as_deref(), &data_root, &own),
    })
}
