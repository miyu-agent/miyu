//! 载入（`docs/designs/07-存储.md` 第七节）：打开日志，照第一条的策略哈希取回快照，交给内核从日志重建，再起 actor。换过
//! 快照的（施工 P-1 再补）照最近换上的那一份。

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use miyu_kernel::facts::Environment;
use miyu_kernel::session::{Input, Session};
use miyu_store::blob::Blobs;
use miyu_store::recall::NO_PERSONA;
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
use crate::guard::Guard;
use crate::handle::{Handle, Ids};
use crate::job_ids::JobIds;
use crate::knowledge::{Shelves, Shelving};
use crate::lettering::Lettering;
use crate::memory::{self, TurnLane, connect};
use crate::port::ForSession;
use crate::report::{Reporter, Upstream, wake_children};
use crate::skills;
use crate::spawn::Lineage;
use crate::store::{Indexed, LogDir};
use crate::tools::ToolKit;

use super::{Load, LoadError, Workplace};
use crate::default_workspace::DefaultWorkspace;

mod stream;

/// 从磁盘载入一个会话：打开日志（自检、截尾），照第 1 条的策略哈希取快照、造策略，交给内核载入。
/// 内核吐出来的动作照样回：有计划的重启打断了的一轮，接着干。
///
/// # Errors
///
/// 日志打不开或者坏了、快照取不出来或者读不懂、内核载入不了。
pub async fn load(setup: Load<'_>) -> Result<Handle, LoadError> {
    // 装箱再等，同 `create`：载入这一段的 future 有近二十 KB。
    Box::pin(load_placed(setup)).await.map(|(handle, _)| handle)
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
        schedules,
        hooks,
        jobs,
        index,
        usage,
        configs,
        knowledge,
        vectors,
        installed,
        plugins,
        memory,
        presets,
        confined: shut_in,
    } = setup;
    let span = actor::span(&id);
    let loading = Instant::now();
    let dir = root.session_dir(&id);
    // 会话日志的目录只交给主人本人的会话（施工 E-M，U25）：关不关照日志里记的（施工 SU-4 中），读完第一条才知道。
    let owner_log = dir.clone();
    let log_dir = LogDir(dir.clone());
    let blobs = Blobs::new(root.blobs());
    let store = blobs.clone();
    let (watching, shipped, stored) = (personas.clone(), resources.clone(), blobs.clone());
    let (table, jobs_dir) = (Arc::clone(jobs), dir.clone());
    let (owner_of, id_of) = (owner.clone(), id.clone());
    let place_resources = shipped.clone();
    let parents = root.clone();
    let (
        log,
        (loaded, base, scan),
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
        // 载入各段用了多久（施工 V-2 中）：读、解日志（施工 V-15 起连着过账本），接上记忆，记进 `loaded` 那一行。
        let began = Instant::now();
        let stream::Read {
            log,
            created: (created, command),
            base,
            snapshot: (snapshot, policy),
            loaded,
            scan,
            snapshotted,
        } = stream::read(&dir, &id_of, &store, &parents)?;
        let read = began.elapsed().saturating_sub(snapshotted);
        let snapshotting = Instant::now();
        let texts = snapshot.driver_texts().map_err(LoadError::Policy)?;
        let run = snapshot.run_texts().map_err(LoadError::Policy)?;
        let guard = snapshot.guard_texts().map_err(LoadError::Policy)?;
        // 关不关（施工 SU-4 中）：日志里记了的照它；以前的日志没记，照造它的一方交的（属主不是主人的关起来）。
        let shut = shut_in || created.confined;
        let place = place(&created.venue, &place_resources, shut).map_err(LoadError::Shipped)?;
        // 能选的池照快照读回（施工 8-8 补）：造会话时拼的那一份，不重拼。
        let pools = Agents::pools_in(&snapshot.tools);
        // 快照里的范围已经照预设、有没有人格算过（施工 P-2 中、P-4 上），这里只再管子会话；分叉出来的支线照父会话的
        // （施工 R-13 三补）。
        let persona = snapshot.persona.as_deref().unwrap_or(NO_PERSONA);
        let scope = memory::scope(memory::subagent(&created), true, snapshot.memory_scope());
        let snapshotted = snapshotted + snapshotting.elapsed();
        let connecting = Instant::now();
        let laid = scan.laid();
        let turns = connect(
            memory.as_ref(),
            scope,
            &owner_of,
            &owner_of,
            (persona, &personas),
            (&id_of, created.parent.is_none()),
            (&created.venue, shut),
            (
                memory::Laid::Read(&laid),
                memory::Source::of(&log, base.clone()),
            ),
        );
        let remembered = scan.remembered();
        Ok((
            log,
            (loaded, base, scan),
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
    let images = created.images;
    let room = crate::memory::room_cell(calls.as_ref());
    // fork 子代理（施工 A-F 中）：是派出去的子代理，照子代理回报、不问人、不召回不记；快照一直是父会话那一份，不重筛。
    let fork_child = crate::memory::fork_child(&created);
    // 分叉出来的支线不是任务，不交回报（施工 C-8 上）；fork 子代理是。
    let upstream = match created.forked.as_ref().filter(|_| !fork_child) {
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
    let tasked = created
        .parent
        .clone()
        .filter(|_| crate::memory::subagent(&created));
    // 换预设时重新筛工具面要的（施工 P-2 下）：场所、父会话和第几层，照 `session.created`。
    let refresh_venue = created.venue.clone();
    let refresh_lineage = tasked.clone().map(|parent| Lineage {
        parent,
        depth: created.depth.unwrap_or(1),
        workspace_from: created.workspace_from.clone(),
    });
    let asks = Agents::asks(&created.venue, tasked.as_ref(), attended);
    let has_goals = Agents::lists_sessions(&created.venue, tasked.as_ref());
    // 能选的人格同池，照快照读回（施工 P-2 补）。
    let agents_personas = Agents::personas_in(&snapshot.tools);
    let on_face = snapshot
        .tools
        .iter()
        .any(|tool| tool.name == miyu_tool::GOAL);
    let loops = snapshot
        .tools
        .iter()
        .any(|tool| tool.name == miyu_tool::SCHEDULE_WAKEUP);
    // 默认工作区用哪个会话的（施工 5-14）：照 `session.created` 记的，没记的用自己的。
    let default_workspace =
        DefaultWorkspace::new(root, created.workspace_from.as_ref().unwrap_or(&id));
    let agents = sessions.map(|port| {
        Arc::new(Agents {
            port,
            session: id.clone(),
            owner: owner.clone(),
            root: root.clone(),
            venue: created.venue,
            depth: tasked.as_ref().map_or(0, |_| created.depth.unwrap_or(0)),
            parent: tasked.clone(),
            workspace_from: created.workspace_from.clone(),
            attended,
            reports: policy.reports.clone(),
            pools,
            preset: created.preset.clone(),
            images,
            confined: place.1.is_some(),
            personas: agents_personas,
            resources: resources.clone(),
            forked: fork_child,
        })
    });
    let kept = blobs.clone();
    models.ready().await;
    // 系统时间比日志里最后一条还早（往回拨过），照最后一条的：时刻不往回走。
    let mut clock = scan.last.map_or_else(Clock::default, Clock::since);
    let count = scan.count;
    let scanning = Instant::now();
    // 她看过的文件（施工 4-6 上）、派出去的任务（施工 7-4）、最近发给了谁（施工 8-8）、累计的用量和计数（施工 9-6 上）、长期
    // 目标（施工 X-6）读日志时一路记下了（施工 V-15）：订阅的回应照它答，之后每落一批盘加上这一批。
    let seen = scan.seen.seen();
    let (roster, sent, tally, goals) = (scan.roster, scan.sent, scan.tally, scan.goals);
    let scanned = scanning.elapsed();
    let replaying = Instant::now();
    let placed = environment.clone();
    let (mut session, first) = match loaded {
        stream::Loaded::Fed(loader) => loader.finish(clock.now(), policy, environment),
        stream::Loaded::Whole(events) => match &base {
            Some(base) => {
                Session::load_forked(id.clone(), base, events, clock.now(), policy, environment)
            }
            None => Session::load(id.clone(), events, clock.now(), policy, environment),
        },
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
    let mut limits = model.limits();
    crate::snapshot::stamp_preset(&mut limits, snapshot.preset.as_ref());
    session.handle(Input::Limits(limits));
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
        home: home.map(Path::to_path_buf),
    });
    let (shelves_open, skill_names) = match installed {
        Some(installed) => {
            let root = root.clone();
            let wanted = snapshot.preset.as_ref().map(|pin| pin.id.clone());
            let places = presets.clone();
            let values = config.current().resolved.values();
            let looked = (
                crate::switches::of(&values),
                crate::packages_off::of(&values),
            );
            // 别的目录照这时的配置（施工 PT-2 外）：关着的不碰。
            let external = miyu_store::skills::external::of(&values, home);
            let site = venue.clone();
            blocking(move || {
                let file = wanted
                    .zip(places)
                    .and_then(|(id, places)| places.file(&id, (images, site.as_str()), looked));
                let found = skills::find(&installed(), &root, &external);
                let found = skills::opened(found, file.as_ref());
                (skills::shelves(&found), skills::names(&found))
            })
            .await
        }
        None => (Vec::new(), skills::Names::new()),
    };
    let guard = Guard::new(
        tools.clone(),
        root.path().to_path_buf(),
        home.map(Path::to_path_buf),
        Arc::clone(&lettering),
        (sandbox.is_some(), place.0, place.1),
    )
    .with_shelves(shelves_open.clone())
    .for_session(root, &id, shipped.path())
    .with_workplace(default_workspace.clone());
    let ledger = crate::usage::Ledger::of(usage.as_ref(), &id);
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
                recall: crate::knowledge::recall::texts(&shipped),
            })
            .await,
        ),
        None => None,
    };
    let mut actor = Actor::new(
        session,
        Box::new({
            // 回合索引等重放完了才接上（施工 V-9）：后台读日志、开库不和载入抢。
            let turns = turns.map(TurnLane::started);
            let indexed = Indexed::new(log, &id, index, usage.map(|usage| (usage, who)), turns);
            match base {
                Some(base) => indexed.on_base(base),
                None => indexed,
            }
        }),
        model,
        ToolKit {
            plugins: plugins.clone(),
            session: id.clone(),
            owner: owner.clone(),
            confined,
            venue: venue.clone(),
            session_log: (!confined).then_some(owner_log),
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
            attachments: miyu_store::attachments::dir(root, &id),
            workplace: Some(default_workspace.clone()),
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
    actor.count_from(tally, goals);
    let busy = actor.busy();
    let watched = actor.watched();
    let shown = actor.shown();
    let on_person = actor.waiting();
    if let Some(upstream) = upstream {
        actor.report_to(Reporter::start(upstream, span.clone()));
    }
    // 文件改动的版本（施工 OV-1 下）：照这一刻的号起，和 Handle 共用。
    let prefix = std::sync::Arc::new(crate::prefix::Prefix::new(
        presets.as_ref().map_or(0, crate::open::PresetPlaces::edits),
    ));
    actor.watch_persona(Refresh {
        personas: watching,
        resources: shipped,
        blobs: stored,
        snapshot,
        child: tasked.is_some(),
        presets,
        tools: tools.clone(),
        prefix: std::sync::Arc::clone(&prefix),
        // 快照是以前拼的，照的目录不知道是哪一代：第一个回合照现在的对一次（施工 O-2 中）。
        seen: None,
        venue: refresh_venue,
        lineage: refresh_lineage,
        knowledge: shelves,
        skills: watch,
        images,
        projects: root.path().join(miyu_store::projects::FILE),
        room: room.clone(),
        plugins,
        frozen: fork_child,
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
    let ids = Ids {
        id,
        venue,
        owner,
        workspace: default_workspace.dir(),
    };
    let handle = Handle::new(ids, inbox, busy, created.oneshot, watched, shown)
        .with_memory(room)
        .with_goals(on_face)
        .with_confined(confined)
        .with_loops(loops)
        .with_waiting(on_person)
        .with_prefix(prefix);
    Ok((handle, placed))
}

/// 毫秒，一位小数：运行日志里写载入各段用了多久（施工 V-2 中）。
fn millis(took: Duration) -> f64 {
    (took.as_secs_f64() * 10_000.0).round() / 10.0
}
