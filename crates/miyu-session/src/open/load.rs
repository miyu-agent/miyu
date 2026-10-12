//! 载入（`docs/designs/07-存储.md` 第七节）：打开日志，照第一条的策略哈希取回快照，交给内核从日志重建，再起 actor。换过
//! 快照的（施工 P-1 再补）照最近换上的那一份。

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use miyu_kernel::event::{Body, Event};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::ContentHash;
use miyu_kernel::origin::Model;
use miyu_kernel::session::{Input, Session};
use miyu_policy::Snapshot;
use miyu_store::blob::Blobs;
use miyu_store::log::{SEGMENT_LIMIT, SessionLog, read_events};
use miyu_store::usage::Who;
use miyu_tool::Log;

use super::place;
use crate::TARGET;
use crate::actor::persona::Refresh;
use crate::actor::{self, Actor, JobKit};
use crate::agents::{Agents, job_in};
use crate::blocking::blocking;
use crate::clock::Clock;
use crate::config::Turning;
use crate::current::Tally;
use crate::effects;
use crate::goal::Goals;
use crate::guard::Guard;
use crate::handle::{Handle, Ids};
use crate::job_ids::JobIds;
use crate::jobs::Roster;
use crate::knowledge::{Shelves, Shelving};
use crate::lettering::Lettering;
use crate::memory::{self, connect};
use crate::port::ForSession;
use crate::report::{Reporter, Upstream, wake_children};
use crate::skills;
use crate::spawn::Lineage;
use crate::store::{Indexed, LogDir};
use crate::tools::ToolKit;

use super::{Load, LoadError, Workplace, ledger_of};

/// 从磁盘载入一个会话：打开日志（自检、截尾），照第 1 条的策略哈希取快照、造策略，交给内核载入。
/// 内核吐出来的动作照样回：有计划的重启打断了的一轮，接着干。
///
/// # Errors
///
/// 日志打不开或者坏了、快照取不出来或者读不懂、内核载入不了。
pub async fn load(setup: Load<'_>) -> Result<Handle, LoadError> {
    load_placed(setup).await.map(|(handle, _)| handle)
}

/// 同 [`load`]，另交回会话这时在哪干活（施工 V-2 三补）：照日志里最后一次记下的挑的，会话表照它记，不用再问 actor。
///
/// # Errors
///
/// 同 [`load`]。
pub async fn load_placed(setup: Load<'_>) -> Result<(Handle, Environment), LoadError> {
    let loaded = open(setup).await?;
    // 载入读、解的整份日志只留最近一次压缩以后的（23 F3，施工 V-2 再补）：别的放掉了，还给系统，不等会话退下。
    drop(tokio::task::spawn_blocking(miyu_heap::trim));
    Ok(loaded)
}

/// 载入，读、解日志用的都在这里放掉。
async fn open(setup: Load<'_>) -> Result<(Handle, Environment), LoadError> {
    let Load {
        root,
        owner,
        personas,
        resources,
        id,
        place: placing,
        models,
        tools,
        home,
        sandbox,
        sandbox_cache,
        sessions,
        packages,
        jobs,
        index,
        usage,
        configs,
        knowledge,
        vectors,
        installed,
        memory,
        presets,
        owner_is_admin,
    } = setup;
    let span = actor::span(&id);
    let loading = Instant::now();
    let dir = root.session_dir(&owner, &id);
    let log_dir = LogDir(dir.clone());
    let blobs = Blobs::new(root.blobs(&owner));
    let store = blobs.clone();
    let (watching, shipped, stored) = (personas.clone(), resources.clone(), blobs.clone());
    let (table, jobs_dir) = (Arc::clone(jobs), dir.clone());
    let (owner_of, id_of) = (owner.clone(), id.clone());
    let place_resources = shipped.clone();
    let (parents, parent_owner) = (root.clone(), owner.clone());
    let (
        log,
        (events, base),
        (created, command),
        (snapshot, pools),
        policy,
        texts,
        run,
        (guard, place),
        wired,
        (read, snapshotted, connected),
        remembered,
    ) = blocking(move || {
        // 载入各段用了多久（施工 V-2 中）：读、解日志，接上记忆，记进 `loaded` 那一行。
        let began = Instant::now();
        let (log, events) = SessionLog::open(&dir, SEGMENT_LIMIT).map_err(LoadError::Log)?;
        let read = began.elapsed();
        let snapshotting = Instant::now();
        let (created, command) = match events.first() {
            Some(Event {
                body: Body::SessionCreated(created),
                cause,
                ..
            }) => (created.clone(), cause.clone()),
            _ => return Err(LoadError::NotCreated),
        };
        // 分叉出来的（施工 C-8 上）：父会话的日志里到分叉点为止的是底子，照只读读（父会话可能正在写）。
        let base = match &created.forked {
            Some(forked) => {
                let parent_dir = parents.session_dir(&parent_owner, &forked.from);
                let parent_log = read_events(&parent_dir).map_err(LoadError::Log)?;
                let base: Vec<Event> = parent_log
                    .into_iter()
                    .take_while(|event| event.seq <= forked.upto)
                    .collect();
                Some(Arc::new(base))
            }
            None => None,
        };
        let bytes = store
            .get(current_policy(&events).unwrap_or(&created.policy))
            .map_err(LoadError::Blob)?;
        let snapshot = Snapshot::from_bytes(&bytes).map_err(LoadError::Snapshot)?;
        let policy = snapshot.policy().map_err(LoadError::Policy)?;
        let texts = snapshot.driver_texts().map_err(LoadError::Policy)?;
        let run = snapshot.run_texts().map_err(LoadError::Policy)?;
        let guard = snapshot.guard_texts().map_err(LoadError::Policy)?;
        let place =
            place(&created.venue, &place_resources, owner_is_admin).map_err(LoadError::Shipped)?;
        // 能选的池照快照读回（施工 8-8 补）：造会话时拼的那一份，不重拼。
        let pools = Agents::pools_in(&snapshot.tools);
        // 快照里的范围已经照预设、有没有人格算过（施工 P-2 中、P-4 上），这里只再管子会话；分叉出来的支线照父会话的
        // （施工 R-13 三补）。
        let persona = snapshot.persona.as_deref().unwrap_or_default();
        let scope = memory::scope(memory::subagent(&created), true, snapshot.memory_scope());
        let snapshotted = snapshotting.elapsed();
        let connecting = Instant::now();
        let turns = connect(
            memory.as_ref(),
            scope,
            &personas.memory_account(persona, &owner_of),
            &owner_of,
            persona,
            &id_of,
            &created.venue,
            (base.as_deref().map_or(&[][..], Vec::as_slice), &events),
        );
        let remembered = remembered(&events);
        Ok((
            log,
            (events, base),
            (created, command),
            (snapshot, pools),
            policy,
            texts,
            run,
            (guard, place),
            turns,
            (read, snapshotted, connecting.elapsed()),
            remembered,
        ))
    })
    .await?;
    // 在哪干活（施工 V-2 三补）：照日志里最后一次记下的，交给会话表挑；会话表原来在载入之前另读一遍整份日志认它。
    let environment = match placing {
        Workplace::Given(environment) => environment,
        Workplace::Remembered { offset, pick } => {
            let (last, dirs) = remembered;
            Environment {
                offset,
                cwd: pick(last.as_deref()),
                dirs,
            }
        }
    };
    let offset = environment.offset;
    let config = Turning::start(configs, environment.cwd.clone()).await;
    let (turns, calls) = wired;
    let attended = snapshot.attended;
    let preset_id = created.preset.clone();
    let room = calls.as_ref().map(|calls| calls.room().clone());
    // 分叉出来的不是任务，不交回报（施工 C-8 上）。
    let upstream = match created.forked {
        Some(_) => None,
        None => Upstream::of(
            sessions.as_ref(),
            created.parent.as_ref(),
            command.as_ref(),
            &id,
        ),
    };
    let port = sessions.clone();
    let who = Who::of(&created);
    let venue = created.venue.clone();
    // 分叉出来的有父会话，可它不是派出去做事的（施工 C-8 上）：子代理那一套（换快照时接的子代理说明、派子代理的层数、问不
    // 问人）照主会话办，快照和父会话的一样，请求的前缀才对得上。
    let tasked = created.parent.clone().filter(|_| created.forked.is_none());
    // 换预设时重新筛工具面要的（施工 P-2 下）：场所、父会话和第几层，照 `session.created`。
    let refresh_venue = created.venue.clone();
    let refresh_lineage = tasked.clone().map(|parent| Lineage {
        parent,
        depth: created.depth.unwrap_or(1),
    });
    let asks = Agents::asks(&created.venue, tasked.as_ref(), attended);
    let has_goals = Agents::lists_sessions(&created.venue, tasked.as_ref());
    // 能选的人格同池，照快照读回（施工 P-2 补）。
    let agents_personas = Agents::personas_in(&snapshot.tools);
    let on_face = snapshot
        .tools
        .iter()
        .any(|tool| tool.name == miyu_tool::GOAL);
    let agents = sessions.map(|port| {
        Arc::new(Agents {
            port,
            session: id.clone(),
            owner: owner.clone(),
            venue: created.venue,
            depth: tasked.as_ref().map_or(0, |_| created.depth.unwrap_or(0)),
            parent: tasked.clone(),
            attended,
            reports: policy.reports.clone(),
            pools,
            preset: created.preset.clone(),
            personas: agents_personas,
        })
    });
    let kept = blobs.clone();
    models.ready().await;
    // 系统时间比日志里最后一条还早（往回拨过），照最后一条的：时刻不往回走。
    let mut clock = events
        .last()
        .map_or_else(Clock::default, |event| Clock::since(event.at));
    let count = events.len();
    let scanning = Instant::now();
    // 她看过的文件（施工 4-6 上）、派出去的任务（施工 7-4）、最近发给了谁（施工 8-8）从日志里重建：内核收走日志之前。
    let seen = match &base {
        Some(base) => effects::seen_in(&[base.as_slice(), events.as_slice()].concat()),
        None => effects::seen_in(&events),
    };
    let roster = Roster::from_events(&events);
    let sent = last_sent(&events);
    // 累计的用量和计数（施工 9-6 上）、长期目标（施工 X-6）：订阅的回应照它答，之后每落一批盘加上这一批。
    let tally = Tally::of(&events);
    let goals = Goals::of(&events);
    let scanned = scanning.elapsed();
    let replaying = Instant::now();
    let placed = environment.clone();
    let (mut session, first) = match &base {
        Some(base) => {
            Session::load_forked(id.clone(), base, events, clock.now(), policy, environment)
        }
        None => Session::load(id.clone(), events, clock.now(), policy, environment),
    }
    .map_err(LoadError::Kernel)?;
    let replayed = replaying.elapsed();
    // 路由照内核从日志算的引用造（施工 8-10）：换过模型的是换过以后的。
    let model = models.port(ForSession {
        id: id.clone(),
        owner: owner.clone(),
        config: Arc::clone(config.current()),
        texts,
        blobs,
        reference: session.reference().map(str::to_string),
        sent,
    });
    // 重启以后接着干的那一轮，发主请求之前就知道限额（施工 6-3 上）；给头看的限额同上（施工 6-3 补）。检查点重读过的
    // 文件，内核在载入吐出来的动作里第一个要回原文（施工 6-9），actor 起来先做它。
    session.handle(Input::Limits(model.limits()));
    // 子会话领的号带上它在父会话里的编号，照 `session.created` 的 `cause` 读回（施工 7-1 补）。
    let prefix = created
        .parent
        .as_ref()
        .zip(command.as_ref())
        .and_then(|(parent, command)| job_in(parent, command));
    let job_ids = Arc::new(JobIds::starting_after(prefix, session.last_job_number()));
    let jobs = JobKit {
        table,
        dir: jobs_dir,
        blobs: kept.clone(),
        ids: Arc::clone(&job_ids),
        roster,
        agents: agents.clone(),
    };
    let waiting = session.waiting_children();
    let (inbox, mailbox) = mpsc::unbounded_channel();
    let lettering = Arc::new(Lettering::new(run, guard));
    // 外部身份的会话（施工 5-12）：权限策略、命令的沙盒都照它。
    let confined = place.1.is_some();
    // 技能的文件夹照这一刻找到的、会话的预设开着的放开只读（施工 X-10 上、X-11 上）；列出来的那一块照快照。
    let watch = installed.clone().map(|installed| skills::Watch {
        installed,
        root: root.clone(),
        owner: owner.clone(),
    });
    let shelves_open = match installed {
        Some(installed) => {
            let (root, owner) = (root.clone(), owner.clone());
            let wanted = snapshot.preset.as_ref().map(|pin| pin.id.clone());
            let places = presets.as_ref().map(|places| places.presets.clone());
            blocking(move || {
                let file = wanted
                    .zip(places)
                    .and_then(|(id, presets)| presets.find(&id).ok())
                    .map(|found| found.file);
                let found =
                    skills::opened(skills::find(&installed(), &root, &owner), file.as_ref());
                skills::shelves(&found)
            })
            .await
        }
        None => Vec::new(),
    };
    let guard = Guard::new(
        tools.clone(),
        root.path().to_path_buf(),
        home.map(Path::to_path_buf),
        Arc::clone(&lettering),
        (sandbox.is_some(), place.0, place.1),
    )
    .with_shelves(shelves_open.clone());
    let ledger = ledger_of(usage.as_ref(), &id, &owner);
    // 知识库照记忆的账号、快照里的人格查（施工 K-1 下）；工具面照快照，第一个回合照现在的目录再判。
    let shelves = match knowledge {
        Some(knowledge) => Some(
            Shelves::loaded(Shelving {
                knowledge,
                persona: snapshot.persona.clone(),
                personas: watching.clone(),
                owner: owner.clone(),
                home: home.map(Path::to_path_buf),
                vectors,
            })
            .await,
        ),
        None => None,
    };
    let mut actor = Actor::new(
        session,
        Box::new({
            let indexed = Indexed::new(log, &id, index, usage.map(|usage| (usage, who)), turns);
            match base {
                Some(base) => indexed.on_base(base),
                None => indexed,
            }
        }),
        model,
        ToolKit {
            session: id.clone(),
            owner: owner.clone(),
            confined,
            catalog: tools.clone(),
            lettering,
            home: home.map(Path::to_path_buf),
            data_root: root.path().to_path_buf(),
            blobs: kept,
            seen,
            sandbox: sandbox.map(Path::to_path_buf),
            sandbox_cache,
            log: Log::new(log_dir),
            offset,
            job_ids,
            agents,
            ledger,
            asks,
            goals: has_goals,
            memory: calls,
            knowledge: shelves.clone(),
            shelves: shelves_open,
            packages,
            preset: preset_id,
        },
        jobs,
        guard,
        mailbox,
        clock,
        config,
    );
    actor.count_from(tally, goals);
    let busy = actor.busy();
    let watched = actor.watched();
    let shown = actor.shown();
    if let Some(upstream) = upstream {
        actor.report_to(Reporter::start(upstream, span.clone()));
    }
    actor.watch_persona(Refresh {
        personas: watching,
        resources: shipped,
        blobs: stored,
        snapshot,
        child: tasked.is_some(),
        presets,
        tools: tools.clone(),
        // 快照是以前拼的，照的目录不知道是哪一代：第一个回合照现在的对一次（施工 O-2 中）。
        seen: None,
        venue: refresh_venue,
        lineage: refresh_lineage,
        knowledge: shelves,
        skills: watch,
    });
    span.in_scope(|| {
        tracing::info!(
            target: TARGET,
            events = count,
            read_ms = millis(read),
            snapshot_ms = millis(snapshotted),
            memory_ms = millis(connected),
            scan_ms = millis(scanned),
            replay_ms = millis(replayed),
            total_ms = millis(loading.elapsed()),
            "loaded"
        );
    });
    if let Some(port) = &port {
        wake_children(port, waiting, &span);
    }
    actor::spawn(actor, first, span);
    let ids = Ids { id, venue, owner };
    let handle = Handle::new(ids, inbox, busy, created.oneshot, watched, shown)
        .with_memory(room)
        .with_goals(on_face);
    Ok((handle, placed))
}

/// 现在的快照（施工 P-1 再补）：整份日志里最近一条带 `policy` 的 `session.policy_changed`，撤掉的回合里的也算（换快照不是
/// 对话的一部分）；没换过的没有，照 `session.created` 的。
pub(crate) fn current_policy(events: &[Event]) -> Option<&ContentHash> {
    events.iter().rev().find_map(|event| match &event.body {
        Body::PolicyChanged(changed) => changed.policy.as_ref(),
        _ => None,
    })
}

/// 最近一条发出去了的 `model.called` 发给了谁（施工 8-8）：钉住的池载入时照它认钉着的成员（「起草时定的」第 2 条）。
fn last_sent(events: &[Event]) -> Option<Model> {
    events.iter().rev().find_map(|event| match &event.body {
        Body::ModelCalled(called) => Some(Model {
            endpoint: called.endpoint.clone()?,
            model: called.model.clone()?,
        }),
        _ => None,
    })
}

/// 毫秒，一位小数：运行日志里写载入各段用了多久（施工 V-2 中）。
fn millis(took: Duration) -> f64 {
    (took.as_secs_f64() * 10_000.0).round() / 10.0
}

/// 日志里最后一次记下的工作目录、加进来的目录（施工 4-9 再补三上、5-10 上、9-7 上；V-2 三补从会话表挪来）：工作目录照列会话的
/// 同一个认法（`miyu_store::index::cwd`：最后一条带 `cwd` 的 `turn.started`、换工作区，没有就照 `session.created`）；加进来的
/// 目录照最后一条 `turn.started` 的，换工作区写了的照它，都没有的是空的。
pub(super) fn remembered(events: &[Event]) -> (Option<String>, Vec<String>) {
    let cwd = events
        .iter()
        .rev()
        .find_map(|event| miyu_store::index::cwd(event).map(str::to_string));
    let dirs = events
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::TurnStarted(started) => Some(started.dirs.clone()),
            Body::WorkspaceChanged(changed) => changed.dirs.clone(),
            _ => None,
        })
        .unwrap_or_default();
    (cwd, dirs)
}
