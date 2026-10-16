//! 会话的几个方法（施工 L-2 时从 `methods.rs` 挪出来：路由改成每个方法一个 future 以后那一份过了 500 行）：造会话、说话、打断、
//! 撤销、恢复、重做、手动压缩、切权限级别、回答提问、清空、回顾、停任务、换模型、改标题、删除。

use super::*;

/// `session.create`：造会话。
pub(super) async fn create(
    core: &Arc<Core>,
    peer: Peer,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    let params: CreateParams = params(request)?;
    // 解析不出的什么都不造（施工 8-8）。
    let model = params
        .model
        .map(|text| models::record(core, &text))
        .transpose()?;
    let memory = params
        .memory
        .map(|text| MemoryScope::parse(&text).ok_or(Refusal::BAD_PARAMS))
        .transpose()?;
    let who = Opening {
        owner: caller.account.clone(),
        attended: peer.input,
        oneshot: params.oneshot,
        model,
        venue: None,
        memory,
        preset: params.preset,
        group: false,
        chosen: params.chosen,
        profile: false,
        images: params.images,
    };
    let created = core
        .sessions
        .create(
            core,
            request.id.clone(),
            params.persona.as_ref().map(Option::as_deref),
            params.cwd,
            params.dirs,
            who,
        )
        .await?;
    let mut reply = json!({"session": created.id.as_str(), "events": [1], "cwd": created.cwd});
    if let Some(file) = created.untrusted {
        reply["untrusted_project"] = json!(file);
    }
    if created.wide {
        reply["wide"] = json!(true);
    }
    Ok(reply)
}

/// `session.send`：说话。
pub(super) async fn send(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    let params: SendParams = params(request)?;
    // 别的 harness 报的名字先查（施工 7-10）：不对的，会话里什么都不送。代表外部的人（施工 O-3）和它不能同时写。
    if params.from.is_some() && params.as_external.is_some() {
        return Err(Refusal::BAD_PARAMS);
    }
    let mut by = match &params.from {
        Some(name) => from::harness(name)?,
        None => admin(core),
    };
    let mut blocks = said(params.text);
    let session = session(&params.session)?;
    // 场所的那几格（施工 O-13 上）：先查写法，只跟着 `as` 来。
    let venue = params
        .venue
        .map(crate::venues::VenueMessageParams::checked)
        .transpose()?;
    if venue.is_some() && params.as_external.is_none() {
        return Err(Refusal::BAD_PARAMS);
    }
    // 附件先查，再找会话：不对的，会话里什么都不送（施工 3-9 三补）。头报的 `cwd`、`dirs` 不再换工作区（施工 9-7 上）。
    let attachments = params.attachments.unwrap_or_default();
    blocks.extend(attach::blocks(core, attachments).await?);
    let found = core.sessions.get(core, &session).await?;
    // 附件照连接的账号读，她照会话的属主读：不是一个账号的拷一份过去（施工 O-4 下）。
    attach::hand_over(core, found.handle.owner(), &blocks).await?;
    // 没有本机路径的落成会话里的文件，她拿得到路径（施工 3-9 六补）。
    attach::keep(core, found.handle.owner(), &session, &mut blocks).await?;
    let command = Command::Send {
        blocks,
        urgent: params.urgent,
        venue,
    };
    // 场所会话只收代表外部的人说的话，本机的会话不收（施工 O-3，`venues.md`）。
    let local = found.handle.venue().as_str() == list::LOCAL;
    match (local, params.as_external) {
        (true, None) => {}
        (true, Some(_)) => return Err(Refusal::BAD_PARAMS),
        (false, None) => return Err(Refusal::VENUE_SESSION),
        (false, Some(speaking)) => {
            by = venues::speaker(core, found.handle.venue(), found.handle.owner(), speaking)?;
        }
    }
    let events = command_by(core, request, &session, &found.handle, by, command).await?;
    let mut reply = json!({"events": events, "cwd": found.cwd});
    let untrusted = core.config().untrusted(&found.cwd);
    if let Some(file) = untrusted {
        reply["untrusted_project"] = json!(file);
    }
    Ok(reply)
}

/// `session.interrupt`：打断。
pub(super) async fn interrupt(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    let params: InterruptParams = params(request)?;
    let queued = match params.queued {
        QueuedParam::Send => Queued::Send,
        QueuedParam::Return => Queued::Return,
        QueuedParam::Keep => Queued::Keep,
    };
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let command = Command::Interrupt { queued };
    let events = command_to(core, request, &session, &found.handle, command).await?;
    Ok(json!({ "events": events }))
}

/// `session.revert`：撤销。
pub(super) async fn revert(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    let params: RevertParams = params(request)?;
    let turn = params
        .turn
        .map(|turn| Seq::new(turn).map(TurnId::new).ok_or(Refusal::BAD_PARAMS))
        .transpose()?;
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let command = Command::Revert { turn };
    let events = command_to(core, request, &session, &found.handle, command).await?;
    Ok(undo::reply(core, &session, &found.cwd, events).await)
}

/// `session.unrevert`：恢复。
pub(super) async fn unrevert(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    let params: UnrevertParams = params(request)?;
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let command = Command::Unrevert;
    let events = command_to(core, request, &session, &found.handle, command).await?;
    Ok(undo::reply(core, &session, &found.cwd, events).await)
}

/// `session.redo`：重做。
pub(super) async fn redo(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    let params: RedoParams = params(request)?;
    let session = session(&params.session)?;
    // 附件照 `session.send` 先查，再找会话。
    let mut attachments = match params.attachments {
        Some(attachments) => Some(attach::blocks(core, attachments).await?),
        None => None,
    };
    let found = core.sessions.get(core, &session).await?;
    if let Some(blocks) = &mut attachments {
        attach::hand_over(core, found.handle.owner(), blocks).await?;
        attach::keep(core, found.handle.owner(), &session, blocks).await?;
    }
    let command = Command::Redo {
        text: params.text.map(said),
        attachments,
    };
    let events = command_to(core, request, &session, &found.handle, command).await?;
    Ok(undo::reply(core, &session, &found.cwd, events).await)
}

/// `session.compact`：手动压缩。
pub(super) async fn compact(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    let params: CompactParams = params(request)?;
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let command = Command::Compact {
        instructions: params.instructions,
    };
    let events = command_to(core, request, &session, &found.handle, command).await?;
    Ok(json!({ "events": events }))
}

/// `session.set_permission_level`：切权限级别。
pub(super) async fn set_permission_level(
    core: &Arc<Core>,
    request: &Request,
) -> Result<Value, Refusal> {
    let params: PermissionParams = params(request)?;
    if params.level.is_none() && params.read_only.is_none() {
        return Err(Refusal::BAD_PARAMS);
    }
    let level = params.level.map(|level| match level {
        LevelParam::Workspace => Level::Workspace,
        LevelParam::Full => Level::Full,
    });
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let command = Command::SetPermission {
        level,
        read_only: params.read_only,
    };
    command_to(core, request, &session, &found.handle, command).await?;
    Ok(json!({}))
}

/// `session.answer`：回答提问。
pub(super) async fn answer_question(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    // 先查参数，再找会话（施工 D-1）。
    let params: AnswerParams = params(request)?;
    let call_id = CallId::parse(&params.call).map_err(|_| Refusal::BAD_PARAMS)?;
    let session = session(&params.session)?;
    let answer = params.answer()?;
    let found = core.sessions.get(core, &session).await?;
    let command = Command::Answer { call_id, answer };
    let events = command_to(core, request, &session, &found.handle, command).await?;
    Ok(json!({ "events": events }))
}

/// `session.clear`：清空上下文。
pub(super) async fn clear(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    let params: ClearParams = params(request)?;
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let events = command_to(core, request, &session, &found.handle, Command::Clear).await?;
    Ok(json!({ "events": events }))
}

/// `session.recap`：回顾。
pub(super) async fn recap(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    let params: RecapParams = params(request)?;
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let by = admin(core);
    match outcome(core, request, &session, &found.handle, by, Command::Recap).await? {
        Outcome::Recapped { text, upto, cached } => {
            Ok(json!({"text": text, "upto": upto.get(), "cached": cached}))
        }
        _ => Err(Refusal::INTERNAL),
    }
}

/// `job.stop`：停掉一个任务。
pub(super) async fn stop_job(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    let params: JobStopParams = params(request)?;
    let session = session(&params.session)?;
    let job = JobId::parse(&params.job).map_err(|_| Refusal::BAD_PARAMS)?;
    let found = core.sessions.get(core, &session).await?;
    match found
        .handle
        .stop_job(job, admin(core), request.id.clone())
        .await
    {
        Ok(Ok(())) => Ok(json!({})),
        Ok(Err(_)) => Err(Refusal::UNKNOWN_JOB),
        Err(_) => {
            core.sessions.forget(&session).await;
            Err(Refusal::STOPPED)
        }
    }
}

/// `session.configure`：换模型。
pub(super) async fn configure(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    let params: models::ConfigureParams = params(request)?;
    let text = params.model()?;
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let model = models::record(core, text)?;
    let command = Command::Configure { model };
    command_to(core, request, &session, &found.handle, command).await?;
    Ok(json!({}))
}

/// `session.set_meta`：改标题、置顶。
pub(super) async fn set_meta(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    let params: MetaParams = params(request)?;
    let command = params.command()?;
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    command_to(core, request, &session, &found.handle, command).await?;
    Ok(json!({}))
}

/// `session.delete`：删除会话。
pub(super) async fn delete(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    let params: DeleteParams = params(request)?;
    let session = session(&params.session)?;
    core.sessions.delete(core, &session).await?;
    Ok(json!({}))
}
