//! 会话的几个方法（施工 L-2 时从 `methods.rs` 挪出来：路由改成每个方法一个 future 以后那一份过了 500 行）：造会话、说话、打断、
//! 退回排着的、撤销、恢复、重做、手动压缩、切权限级别、回答提问、清空、回顾、停任务、换模型、改标题、删除。

use super::*;

mod recast;

/// `session.create`：造会话。
pub(super) async fn create(
    core: &Arc<Core>,
    peer: Peer,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    let mut params: CreateParams = params(request)?;
    // 在项目里建的（施工 H-2）：项目要在，没写的人格、预设照项目的。
    let project = match &params.project {
        Some(id) => Some(crate::projects::sessions::find(core, caller.acting_for(), id).await?),
        None => None,
    };
    if let Some(project) = &project {
        params.persona = params.persona.or_else(|| project.persona.clone());
        params.preset = params.preset.or_else(|| project.preset.clone().map(Some));
    }
    // 解析不出的什么都不造（施工 8-8）。
    let model = params
        .model
        .map(|text| models::record(core, caller.acting_for(), &text))
        .transpose()?;
    let memory = params
        .memory
        .map(|text| MemoryScope::parse(&text).ok_or(Refusal::BAD_PARAMS))
        .transpose()?;
    let who = Opening {
        owner: caller.acting_for().clone(),
        attended: peer.input,
        oneshot: params.oneshot,
        model,
        venue: None,
        memory,
        preset: crate::presets::opening(params.preset)?,
        group: None,
        profile: false,
        images: params.images,
        titles: params.titles,
        origin: None,
        confined: false,
    };
    let created = core
        .sessions
        .create(
            core,
            request.id.clone(),
            params.persona.as_ref().map(Option::as_deref),
            params.cwd.unwrap_or_else(|| {
                let (_, own) = crate::sessions::roots(core, caller.acting_for());
                own.to_string_lossy().into_owned()
            }),
            params.dirs,
            who,
        )
        .await?;
    let mut events = vec![1];
    if let Some(project) = project {
        events.extend(
            crate::projects::sessions::join(
                core,
                crate::sessions::person(core, caller),
                request,
                &created.id,
                project.project,
            )
            .await?,
        );
    }
    let mut reply = json!({"session": created.id.as_str(), "events": events, "cwd": created.cwd});
    if let Some(file) = created.untrusted {
        reply["untrusted_project"] = json!(file);
    }
    if created.wide {
        reply["wide"] = json!(true);
    }
    Ok(reply)
}

/// `session.send`：说话。`caller` 是核心拉起的扩展的，它不带 `as` 说的话记成它自己（施工 X-30）。
pub(super) async fn send(
    core: &Arc<Core>,
    (caller, peer): (&Caller, crate::hello::Peer),
    request: &Request,
) -> Result<Value, Refusal> {
    let params: SendParams = params(request)?;
    let choice = params.prefix;
    // 别的 harness 报的名字先查（施工 7-10）：不对的，会话里什么都不送。代表外部的人（施工 O-3）和它不能同时写。
    if params.from.is_some() && params.as_external.is_some() {
        return Err(Refusal::BAD_PARAMS);
    }
    // 扩展的身份是它自己（`04-核心协议.md` 第四节）：记成名字是包编号的 harness，组装照「别的 harness 发来的话」告诉她这不是人说的；
    // 自己报 `from` 的冒充不了。
    let mut by = match (&params.from, caller.package.as_deref()) {
        (Some(_), Some(_)) => return Err(Refusal::BAD_PARAMS),
        (Some(name), None) => from::harness(name)?,
        (None, Some(package)) => from::harness(package)?,
        (None, None) => crate::sessions::person(core, caller),
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
    // 换工作目录的只给本机的会话（施工 9-7 再补）：代表平台上的人说话的不收。
    if params.cwd.is_some() && params.as_external.is_some() {
        return Err(Refusal::BAD_PARAMS);
    }
    // 附件先查，再找会话：不对的，会话里什么都不送（施工 3-9 三补）。头报的 `cwd`、`dirs` 不再换工作区（施工 9-7 上）。
    let attachments = params.attachments.unwrap_or_default();
    blocks.extend(attach::blocks(core, &caller.account, attachments).await?);
    let mut found = core.sessions.get(core, &session).await?;
    // 附件照连接的账号读，她照会话的属主读：不是一个账号的拷一份过去（施工 O-4 下）。
    attach::hand_over(core, (&caller.account, found.handle.owner()), &blocks).await?;
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
            by = venues::speaker(core, &found.handle, speaking)?;
        }
    }
    // 先换目录再说（施工 9-7 再补）：这一句开的那一轮照新目录跑。
    let wide = match params.cwd.as_deref() {
        Some(cwd) if local => {
            crate::workspace::follow(core, caller, request, &session, &mut found, cwd).await?
        }
        _ => false,
    };
    // 文件改了、前缀要变的，开新一轮之前问人（施工 OV-1 下）。
    if local {
        let asked = (&found.handle, &request.id);
        crate::sessions::prefix::ask(core, (peer, &by), asked, choice).await?;
    }
    let events = command_by(core, request, &session, &found.handle, by, command).await?;
    let mut reply = json!({"events": events, "cwd": found.cwd});
    if wide {
        reply["wide"] = json!(true);
    }
    let untrusted = core
        .mine(found.handle.owner())
        .config()
        .untrusted(&found.cwd);
    if let Some(file) = untrusted {
        reply["untrusted_project"] = json!(file);
    }
    Ok(reply)
}

/// `session.interrupt`：打断。
pub(super) async fn interrupt(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    let params: InterruptParams = params(request)?;
    let queued = match params.queued {
        QueuedParam::Send => Queued::Send,
        QueuedParam::Return => Queued::Return,
        QueuedParam::Keep => Queued::Keep,
    };
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let command = Command::Interrupt { queued };
    let events = command_to(core, caller, request, &session, &found.handle, command).await?;
    Ok(json!({ "events": events }))
}

/// `session.withdraw`：不打断，退回一条排着队、她还没听到的消息（施工 Q-W）。
pub(super) async fn withdraw(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    let params: WithdrawParams = params(request)?;
    let message = params
        .message
        .map(|message| Seq::new(message).ok_or(Refusal::BAD_PARAMS))
        .transpose()?;
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let command = Command::Withdraw { message };
    let events = command_to(core, caller, request, &session, &found.handle, command).await?;
    Ok(json!({ "events": events }))
}

/// `session.revert`：撤销。
pub(super) async fn revert(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    let params: RevertParams = params(request)?;
    let turn = params
        .turn
        .map(|turn| Seq::new(turn).map(TurnId::new).ok_or(Refusal::BAD_PARAMS))
        .transpose()?;
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let command = Command::Revert { turn };
    let events = command_to(core, caller, request, &session, &found.handle, command).await?;
    Ok(undo::reply(core, &session, &found.cwd, events).await)
}

/// `session.unrevert`：恢复。
pub(super) async fn unrevert(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    let params: UnrevertParams = params(request)?;
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let command = Command::Unrevert;
    let events = command_to(core, caller, request, &session, &found.handle, command).await?;
    Ok(undo::reply(core, &session, &found.cwd, events).await)
}

/// `session.redo`：重做。
pub(super) async fn redo(
    core: &Arc<Core>,
    (caller, peer): (&Caller, crate::hello::Peer),
    request: &Request,
) -> Result<Value, Refusal> {
    let params: RedoParams = params(request)?;
    let session = session(&params.session)?;
    // 附件照 `session.send` 先查，再找会话。
    let mut attachments = match params.attachments {
        Some(attachments) => Some(attach::blocks(core, &caller.account, attachments).await?),
        None => None,
    };
    let found = core.sessions.get(core, &session).await?;
    if let Some(blocks) = &mut attachments {
        attach::hand_over(core, (&caller.account, found.handle.owner()), blocks).await?;
        attach::keep(core, found.handle.owner(), &session, blocks).await?;
    }
    // 文件改了、前缀要变的，重发之前问人（施工 OV-1 下）。
    let by = crate::sessions::person(core, caller);
    crate::sessions::prefix::ask(
        core,
        (peer, &by),
        (&found.handle, &request.id),
        params.prefix,
    )
    .await?;
    let command = Command::Redo {
        text: params.text.map(said),
        attachments,
    };
    let events = command_to(core, caller, request, &session, &found.handle, command).await?;
    Ok(undo::reply(core, &session, &found.cwd, events).await)
}

/// `session.compact`：手动压缩。
pub(super) async fn compact(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    let params: CompactParams = params(request)?;
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let command = Command::Compact {
        instructions: params.instructions,
    };
    let events = command_to(core, caller, request, &session, &found.handle, command).await?;
    Ok(json!({ "events": events }))
}

/// `session.set_permission_level`：切权限级别。
pub(super) async fn set_permission_level(
    core: &Arc<Core>,
    caller: &Caller,
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
    // 完全放开只给管理员（施工 Y-2 下下上，2026-10-23 项目主人定：成员最高到工作区）：完全放开的命令不进沙盒，等于碰得到这台电脑。
    if level == Some(Level::Full) && !crate::login::is_owner(core, caller.acting_for()) {
        return Err(Refusal::NOT_PERMITTED);
    }
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let command = Command::SetPermission {
        level,
        read_only: params.read_only,
    };
    command_to(core, caller, request, &session, &found.handle, command).await?;
    Ok(json!({}))
}

/// `session.answer`：回答提问。
pub(super) async fn answer_question(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    // 先查参数，再找会话（施工 D-1）。
    let params: AnswerParams = params(request)?;
    let call_id = CallId::parse(&params.call).map_err(|_| Refusal::BAD_PARAMS)?;
    let session = session(&params.session)?;
    let wanted = params.wanted()?;
    // 附件先查，再找会话：不对的什么都不送（同 `session.send`，施工 Q-A）。
    let mut answer = match wanted {
        Wanted::Approval {
            decision,
            reason,
            attachments,
            nested,
        } => miyu_kernel::session::Answer::Approval {
            decision,
            reason,
            attachments: attach::blocks(core, &caller.account, attachments).await?,
            nested,
        },
        Wanted::Questions(items) => {
            let mut answers = Vec::with_capacity(items.len());
            for (mut response, attachments) in items {
                response.attachments = attach::blocks(core, &caller.account, attachments).await?;
                answers.push(response);
            }
            miyu_kernel::session::Answer::Questions(answers)
        }
    };
    let found = core.sessions.get(core, &session).await?;
    let owner = found.handle.owner();
    let lists: Vec<&mut Vec<miyu_kernel::block::Block>> = match &mut answer {
        miyu_kernel::session::Answer::Approval { attachments, .. } => vec![attachments],
        miyu_kernel::session::Answer::Questions(answers) => answers
            .iter_mut()
            .map(|answer| &mut answer.attachments)
            .collect(),
    };
    for blocks in lists {
        attach::hand_over(core, (&caller.account, owner), blocks).await?;
        attach::keep(core, owner, &session, blocks).await?;
    }
    let command = Command::Answer { call_id, answer };
    let events = command_to(core, caller, request, &session, &found.handle, command).await?;
    Ok(json!({ "events": events }))
}

/// `session.clear`：清空上下文。
pub(super) async fn clear(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    let params: ClearParams = params(request)?;
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let events = command_to(
        core,
        caller,
        request,
        &session,
        &found.handle,
        Command::Clear,
    )
    .await?;
    Ok(json!({ "events": events }))
}

/// `session.recap`：回顾。
pub(super) async fn recap(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    let params: RecapParams = params(request)?;
    let session = session(&params.session)?;
    let found = core.sessions.get(core, &session).await?;
    let by = crate::sessions::person(core, caller);
    match outcome(core, request, &session, &found.handle, by, Command::Recap).await? {
        Outcome::Recapped { text, upto, cached } => {
            Ok(json!({"text": text, "upto": upto.get(), "cached": cached}))
        }
        _ => Err(Refusal::INTERNAL),
    }
}

/// `job.stop`：停掉一个任务。
pub(super) async fn stop_job(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    let params: JobStopParams = params(request)?;
    let session = session(&params.session)?;
    let job = JobId::parse(&params.job).map_err(|_| Refusal::BAD_PARAMS)?;
    let found = core.sessions.get(core, &session).await?;
    match found
        .handle
        .stop_job(
            job,
            crate::sessions::person(core, caller),
            request.id.clone(),
        )
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

/// `session.configure`：换模型；`model` 写 `null` 的回到跟着 `models.chat`（施工 8-10 再补）。换人格、预设（施工 P-13）在
/// `recast.rs`：先查完，模型先换，再换人格、预设。
pub(super) async fn configure(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    let params: models::ConfigureParams = params(request)?;
    let text = params.model()?;
    let session = session(&params.session)?;
    let recast = recast::checked(core, &session, params.recast()).await?;
    let found = core.sessions.get(core, &session).await?;
    // 写 `null` 的回到跟着 `models.chat`：内核记空的（施工 8-10 再补）。
    let model = match text {
        Some(Some(text)) => Some(models::record(core, found.handle.owner(), text)?),
        Some(None) => Some(String::new()),
        None => None,
    };
    if let Some(model) = model {
        let command = Command::Configure { model };
        command_to(core, caller, request, &session, &found.handle, command).await?;
    }
    if let Some(command) = recast {
        recast::send(core, caller, request, &session, &found.handle, command).await?;
    }
    Ok(json!({}))
}

/// `session.set_meta`：改标题、置顶、项目（施工 H-2：移进的项目要在，会话要是本机的主会话）。
pub(super) async fn set_meta(
    core: &Arc<Core>,
    caller: &Caller,
    request: &Request,
) -> Result<Value, Refusal> {
    let params: MetaParams = params(request)?;
    let command = params.command()?;
    let session = session(&params.session)?;
    if let Some(project) = params.project() {
        crate::projects::sessions::find(core, caller.acting_for(), project).await?;
    }
    if params.moves() {
        crate::projects::sessions::may_join(core, &session).await?;
    }
    let found = core.sessions.get(core, &session).await?;
    command_to(core, caller, request, &session, &found.handle, command).await?;
    Ok(json!({}))
}

/// `session.delete`：删除会话。
pub(super) async fn delete(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    let params: DeleteParams = params(request)?;
    let session = session(&params.session)?;
    core.sessions.delete(core, &session).await?;
    Ok(json!({}))
}
