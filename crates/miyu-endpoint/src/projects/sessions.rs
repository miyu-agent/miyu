//! 会话在项目里（施工 H-2，设计 `38-项目.md` 第一节第 2、3 条、第二节第 2、4 条）：哪个会话在哪个项目记在会话自己的日志里
//! （`session.meta_changed` 的 `project`），这里查能不能进、项目在不在，删项目时把里面的会话挪出来。

use std::sync::Arc;

use miyu_kernel::event::Body;
use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::Command;
use miyu_store::projects::Project;
use miyu_tool::Stop;

use crate::Core;
use crate::list::{LOCAL, scan};
use crate::refusal::Refusal;
use crate::wire::Request;

/// 项目 `id`：没有的 `unknown_project`。
pub(crate) async fn find(core: &Core, id: &str) -> Result<Project, Refusal> {
    let read = super::read(core).await?;
    read.projects
        .find(id)
        .cloned()
        .ok_or(Refusal::UNKNOWN_PROJECT)
}

/// 会话 `session` 能不能进项目：只有本机的主会话能（场所会话、子代理、分叉出来的不能：分叉出来的也有父会话），别的是 `data.field` 为 `project`
/// 的参数不对。会话没有、读不了第一条的照会话没有。
pub(crate) async fn may_join(core: &Core, session: &SessionId) -> Result<(), Refusal> {
    if !core.sessions.exists(core, session).await {
        return Err(Refusal::NOT_FOUND);
    }
    let dir = core.root.session_dir(session);
    let first = tokio::task::spawn_blocking(move || miyu_store::log::first_event(&dir))
        .await
        .map_err(|_| Refusal::INTERNAL)?
        .map_err(|_| Refusal::NOT_FOUND)?;
    match first.body {
        Body::SessionCreated(created)
            if created.venue.as_str() == LOCAL && created.parent.is_none() =>
        {
            Ok(())
        }
        _ => Err(Refusal::bad_field("project")),
    }
}

/// 删项目 `id` 之前：在这个项目里的会话一个个写一条 `session.meta_changed {"project": ""}`（`by` 是人），交回挪了
/// 几个。命令编号照这条请求的接 `/project`：一个会话里不会撞。
pub(crate) async fn move_out(
    core: &Arc<Core>,
    request: &Request,
    id: &str,
) -> Result<usize, Refusal> {
    let members = members(core, id).await?;
    let command_id = CommandId::parse(&format!("{}/project", request.id.as_str()))
        .map_err(|_| Refusal::BAD_PARAMS)?;
    for session in &members {
        let found = core.sessions.get(core, session).await?;
        let command = Command::SetMeta {
            title: None,
            pinned: None,
            project: Some(String::new()),
        };
        let by = crate::sessions::person();
        if found
            .handle
            .command(command_id.clone(), by, command)
            .await
            .is_err()
        {
            core.sessions.forget(session).await;
            return Err(Refusal::STOPPED);
        }
    }
    Ok(members.len())
}

/// 在项目 `id` 里的本机会话：照会话列表的那一套扫，照会话列表的索引。
async fn members(core: &Core, id: &str) -> Result<Vec<SessionId>, Refusal> {
    let root = core.root.clone();
    let index = core.index();
    let id = id.to_string();
    let scanned = tokio::task::spawn_blocking(move || {
        scan(
            &root,
            Some(&*index),
            &Default::default(),
            |row| row.venue == LOCAL && row.project.as_deref() == Some(id.as_str()),
            None,
            &Stop::default(),
        )
    })
    .await;
    match scanned {
        Ok(Ok(listed)) => Ok(listed.into_iter().map(|one| one.id).collect()),
        _ => Err(Refusal::INTERNAL),
    }
}

/// 刚建好的会话 `session` 移进项目 `project`（`session.create` 带了 `project` 的）：记一条 `session.meta_changed`，交回它的
/// 序号。命令编号照这条请求的接 `/project`：建会话的那一条已经用了原来的。
pub(crate) async fn join(
    core: &Arc<Core>,
    by: By,
    request: &Request,
    session: &SessionId,
    project: String,
) -> Result<Vec<u64>, Refusal> {
    let command_id = CommandId::parse(&format!("{}/project", request.id.as_str()))
        .map_err(|_| Refusal::BAD_PARAMS)?;
    let found = core.sessions.get(core, session).await?;
    let command = Command::SetMeta {
        title: None,
        pinned: None,
        project: Some(project),
    };
    match found.handle.command(command_id, by, command).await {
        Ok(miyu_kernel::session::Outcome::Accepted { events }) => {
            Ok(events.iter().map(|seq| seq.get()).collect())
        }
        Ok(_) => Err(Refusal::INTERNAL),
        Err(_) => {
            core.sessions.forget(session).await;
            Err(Refusal::STOPPED)
        }
    }
}
