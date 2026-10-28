//! 造会话、载入（`docs/designs/07-存储.md` 第四、七节，施工 3-6 上的策略快照）：备好磁盘上的，交给
//! 内核造会话、或者从日志重建，再起 actor。磁盘上的事都在阻塞线程里做。载入在 `load.rs`（施工 R-3 下挪出去）。

use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use miyu_kernel::event::SessionCreated;
use miyu_kernel::session::{Input, Session};
use miyu_models::provider::chat;
use miyu_store::blob::Blobs;
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};
use miyu_store::recall::NO_PERSONA;
use miyu_store::usage::Who;
use miyu_tool::{Log, Seen};

use crate::TARGET;
use crate::actor::persona::Refresh;
use crate::actor::{self, Actor, JobKit};
use crate::agents::{Agents, job_in};
use crate::blocking::blocking;
use crate::clock::Clock;
use crate::config::Turning;
use crate::default_workspace::DefaultWorkspace;
use crate::guard::Guard;
use crate::handle::{Handle, Ids};
use crate::job_ids::JobIds;
use crate::jobs::Roster;
use crate::lettering::Lettering;
use crate::memory::{self, connect};
use crate::port::ForSession;
use crate::report::{Reporter, Upstream};
use crate::snapshot::build;
use crate::spawn::Lineage;
use crate::store::{Indexed, LogDir};
use crate::tools::ToolKit;
use crate::usage::Ledger;

mod draft;
mod error;
mod face;
mod fork;
mod load;
mod place;
use place::place;
mod scan;
mod setup;

pub use draft::preview_context;
pub use error::{CreateError, LoadError};
pub use face::preview;
pub use fork::{Fork, ForkError, Within, fork};
pub use load::{load, load_placed};
pub(crate) use scan::current_policy;
pub use setup::{
    Create, GroupSettings, Installed, Load, OfferedFeatures, PresetPlaces, ToolOverrides, Workplace,
};

/// 造一个会话：先把策略快照存成 blob（先落 blob，再写引用它的事件），再建会话目录和日志，交给内核
/// 造会话；`session.created` 落了盘，才交回 [`Handle`]。子会话（带着 [`Create::lineage`]）的 system 接上场所说明
/// （施工 7-5）。
///
/// # Errors
///
/// 人格、子会话的场所说明读不出来，策略造不出来、磁盘上建不成；造会话那一条没落盘。
pub async fn create(setup: Create<'_>) -> Result<Handle, CreateError> {
    // 装箱再等：造会话这一段的 future 有三十多 KB，不装箱的话每个调用方的 future 都跟着大，调试构建在 Windows 较深的调用链上
    // 会栈溢出（施工 J-1 上的 CI 在 `route_learned` 上撞到过）。
    Box::pin(created(setup)).await
}

/// [`create`] 干活的那一段。
async fn created(setup: Create<'_>) -> Result<Handle, CreateError> {
    let config = Turning::start(setup.configs.clone(), setup.environment.cwd.clone()).await;
    let mut opened = face::open(&setup, &config.current().resolved.values()).await;
    let taken = (
        std::mem::take(&mut opened.face),
        std::mem::take(&mut opened.listed),
    );
    let draft::Drafted {
        mut parts,
        shelves: shelves_open,
        names: skill_names,
        watch,
    } = draft::draft(&setup, &opened, taken).await?;
    // 给了的场所插件工具记进快照（施工 O-60）。
    parts.plugin_tools = std::mem::take(&mut opened.plugin_tools);
    let face::Opened {
        preset,
        scope,
        shelves,
        edition,
        ..
    } = opened;
    let Create {
        root,
        resources,
        id,
        persona,
        persona_texts: _,
        profile: _,
        personas,
        memory_account,
        memory_scope: _,
        venue,
        owner,
        permission,
        attended,
        oneshot,
        environment,
        command,
        by,
        models,
        tools,
        home,
        sandbox,
        sandbox_cache,
        lineage,
        sessions,
        packages,
        schedules,
        hooks,
        jobs,
        index,
        usage,
        configs: _,
        model,
        knowledge: _,
        vectors: _,
        installed: _,
        plugins,
        memory,
        preset: _,
        no_tools,
        presets,
        group: _,
        confined: shut_in,
        images,
        titles,
        dream,
        origin,
    } = setup;
    let span = actor::span(&id);
    // 没指定的照这时的 `models.chat`：记进 `session.created`，以后照它（施工 8-8）。
    let reference = model.or_else(|| chat(&config.current().resolved.values()));
    let (resources, name) = (resources.clone(), persona.map(str::to_string));
    let shipped = resources.clone();
    let preset = preset.map(|chosen| chosen.id);
    let asks = Agents::asks(
        &venue,
        lineage.as_ref().map(|lineage| &lineage.parent),
        attended,
    );
    let goals = Agents::lists_sessions(&venue, lineage.as_ref().map(|lineage| &lineage.parent));
    let on_face = parts.face.iter().any(|tool| tool.name == miyu_tool::GOAL);
    let loops = parts
        .face
        .iter()
        .any(|tool| tool.name == miyu_tool::SCHEDULE_WAKEUP);
    let pools = Agents::pools_in(&parts.face);
    let agents_personas = Agents::personas_in(&parts.face);
    let child = lineage.is_some();
    let count = parts.face.len();
    let dir = root.session_dir(&id);
    // 会话日志的目录只交给主人本人的会话（施工 E-M，U25）。
    let session_log = (!shut_in).then(|| dir.clone());
    let abandoned = dir.clone();
    let log_dir = LogDir(dir.clone());
    let offset = environment.offset;
    let blobs = Blobs::new(root.blobs());
    let store = blobs.clone();
    let (table, jobs_dir) = (Arc::clone(jobs), dir.clone());
    let (owner_of, id_of, venue_of) = (owner.clone(), id.clone(), venue.clone());
    let place_resources = shipped.clone();
    let place_venue = venue.clone();
    // 那一间里她叫什么照人格的名字记（施工 E-1 修）。
    let persona_book = personas.clone();
    let (snapshot, policy, texts, run, (guard, place), log, (turns, calls)) = blocking(move || {
        let mut snapshot = build(&resources, parts).map_err(CreateError::Persona)?;
        snapshot.no_tools = no_tools;
        let policy = snapshot.policy().map_err(CreateError::Policy)?;
        let texts = snapshot.driver_texts().map_err(CreateError::Policy)?;
        let run = snapshot.run_texts().map_err(CreateError::Policy)?;
        let guard = snapshot.guard_texts().map_err(CreateError::Policy)?;
        let place = place(&place_venue, &place_resources, shut_in).map_err(CreateError::Shipped)?;
        store.put(&snapshot.to_bytes()).map_err(CreateError::Disk)?;
        let log = SessionLog::create(&dir, SEGMENT_LIMIT).map_err(CreateError::Disk)?;
        let turns = connect(
            memory.as_ref(),
            scope,
            &memory_account,
            &owner_of,
            (name.as_deref().unwrap_or(NO_PERSONA), &persona_book),
            (&id_of, !child),
            (&venue_of, place.1.is_some()),
            (memory::Laid::Empty, memory::Source::of(&log, None)),
        );
        Ok((snapshot, policy, texts, run, (guard, place), log, turns))
    })
    .await?;
    let room = crate::memory::room_cell(calls.as_ref());
    let (kept, stored) = (blobs.clone(), blobs.clone());
    models.ready().await;
    let model = models.port(ForSession {
        id: id.clone(),
        owner: owner.clone(),
        config: Arc::clone(config.current()),
        texts,
        blobs,
        reference: reference.clone(),
        sent: None,
    });
    let mut clock = Clock::default();
    let upstream = Upstream::of(
        sessions.as_ref(),
        lineage.as_ref().map(|lineage| &lineage.parent),
        Some(&command),
        &id,
    );
    // 默认工作区用哪个会话的（施工 5-14）：子会话照父会话的，主会话用自己的。
    let workspace_from = lineage.as_ref().map(Lineage::workspace_of);
    let workplace = DefaultWorkspace::new(root, workspace_from.as_ref().unwrap_or(&id));
    let agents = sessions.map(|port| {
        Arc::new(Agents {
            port,
            session: id.clone(),
            owner: owner.clone(),
            root: root.clone(),
            venue: venue.clone(),
            depth: Agents::depth_of(lineage.as_ref()),
            parent: lineage.as_ref().map(|lineage| lineage.parent.clone()),
            workspace_from: workspace_from.clone(),
            attended,
            reports: policy.reports.clone(),
            pools,
            preset: preset.clone(),
            images,
            confined: place.1.is_some(),
            personas: agents_personas,
            resources: shipped.clone(),
            forked: false,
        })
    });
    let preset_id = preset.clone();
    let created = SessionCreated {
        oneshot,
        cwd: Some(environment.cwd.clone()),
        parent: lineage.as_ref().map(|lineage| lineage.parent.clone()),
        depth: lineage.as_ref().map(|lineage| lineage.depth),
        workspace_from,
        model: reference,
        preset,
        images,
        titles,
        dream,
        origin,
        confined: shut_in,
        ..snapshot.session_created(owner.clone(), venue.clone(), permission)
    };
    let (mut session, first) = Session::create(
        id.clone(),
        command.clone(),
        by,
        clock.now(),
        created,
        policy,
        environment,
    );
    // 模型的限额在别的输入之前交（施工 6-3 上）：什么动作都不出。给头看的那一份由 actor 当场要，`Handle` 和它共用（施工
    // 6-3 补；施工 8-9 起会变）。
    let mut limits = model.limits();
    crate::snapshot::stamp_preset(&mut limits, snapshot.preset.as_ref());
    session.handle(Input::Limits(limits));
    // 子会话领的号带上它在父会话里的编号，照造它的命令读回（施工 7-1 补）。
    let prefix = lineage
        .as_ref()
        .and_then(|lineage| job_in(&lineage.parent, &command));
    let job_ids = Arc::new(JobIds::starting_after(prefix, session.last_job_number()));
    let jobs = JobKit {
        table,
        dir: jobs_dir,
        blobs: kept.clone(),
        ids: Arc::clone(&job_ids),
        roster: Roster::default(),
        agents: agents.clone(),
    };
    let (inbox, mailbox) = mpsc::unbounded_channel();
    let lettering = Arc::new(Lettering::new(run, guard));
    // 外部身份的会话（施工 5-12）：权限策略、命令的沙盒都照它。
    let confined = place.1.is_some();
    let guard = Guard::new(
        tools.clone(),
        root.path().to_path_buf(),
        home.map(Path::to_path_buf),
        Arc::clone(&lettering),
        (sandbox.is_some(), place.0, place.1),
    )
    .with_shelves(shelves_open.clone())
    .for_session(root, &id, shipped.path())
    .with_workplace(workplace.clone());
    let who = Who {
        owner: owner.clone(),
        venue: venue.clone(),
        parent: lineage.as_ref().map(|lineage| lineage.parent.clone()),
    };
    let ledger = Ledger::of(usage.as_ref(), &id);
    let mut actor = Actor::new(
        session,
        Box::new(Indexed::new(
            log,
            &id,
            index,
            usage.map(|usage| (usage, who)),
            turns,
        )),
        model,
        ToolKit {
            plugins: plugins.clone(),
            session: id.clone(),
            owner: owner.clone(),
            confined,
            venue: venue.clone(),
            session_log,
            catalog: tools.clone(),
            lettering,
            home: home.map(Path::to_path_buf),
            data_root: root.path().to_path_buf(),
            blobs: kept,
            seen: Seen::new(),
            sandbox: sandbox.map(Path::to_path_buf),
            sandbox_cache,
            log: Log::new(log_dir),
            offset,
            job_ids,
            agents,
            ledger,
            asks,
            goals,
            memory: calls,
            knowledge: shelves.clone(),
            shelves: shelves_open,
            attachments: miyu_store::attachments::dir(root, &id),
            workplace: Some(workplace.clone()),
            names: skill_names,
            packages,
            schedules,
            hooks,
            preset: preset_id,
        },
        jobs,
        guard,
        mailbox,
        clock,
        config,
    );
    if let Some(upstream) = upstream {
        actor.report_to(Reporter::start(upstream, span.clone()));
    }
    let (resources, blobs) = (shipped, stored);
    // 文件改动的版本（施工 OV-1 下）：照这一刻的号起，和 Handle 共用。
    let prefix = std::sync::Arc::new(crate::prefix::Prefix::new(
        presets.as_ref().map_or(0, crate::open::PresetPlaces::edits),
    ));
    actor.watch_persona(Refresh {
        personas,
        resources,
        blobs,
        snapshot,
        child,
        presets,
        tools: tools.clone(),
        prefix: std::sync::Arc::clone(&prefix),
        seen: Some(edition.clone()),
        venue: venue.clone(),
        lineage: lineage.clone(),
        knowledge: shelves,
        skills: watch,
        images,
        projects: root.path().join(miyu_store::projects::FILE),
        room: room.clone(),
        plugins,
        frozen: false,
    });
    let busy = actor.busy();
    let watched = actor.watched();
    let shown = actor.shown();
    let on_person = actor.waiting();
    let (reply, answer) = oneshot::channel();
    actor.wait_for(command, reply, false);
    span.in_scope(|| {
        tracing::info!(target: TARGET, persona, venue = venue.as_str(), tools = count, "created");
    });
    actor::spawn(actor, first, span);
    match answer.await {
        Ok(_) => {
            let ids = Ids {
                id,
                venue,
                owner,
                workspace: workplace.dir(),
            };
            Ok(Handle::new(ids, inbox, busy, oneshot, watched, shown)
                .with_memory(room)
                .with_goals(on_face)
                .with_confined(confined)
                .with_loops(loops)
                .with_waiting(on_person)
                .with_prefix(prefix))
        }
        Err(_) => {
            error::abandoned(abandoned, &id).await;
            Err(CreateError::Stopped)
        }
    }
}

#[cfg(test)]
mod tests;
