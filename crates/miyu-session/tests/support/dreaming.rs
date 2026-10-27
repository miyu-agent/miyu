//! 整理会话的几个测试共用的（施工 D-2）：假的会话表记下开的、每一批的字、每一次叫停、收的时候交来的结果，照剧本交回。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use miyu_http::testkit::Server;
use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::time::UtcOffset;
use miyu_session::{
    DreamBatch, DreamResult, DreamSessions, DreamSpec, Dreamed, Keeper, Listener, NotDreamed,
    Pending, Turn, Watched,
};
use tokio::sync::Notify;

use super::extracting::organizer;
use super::meaning::persona;
use super::{Home, alice, alice_account};

/// 会话表那一头照剧本怎么交回一批。
#[derive(Debug, Clone)]
pub enum Step {
    Answer(String),
    Fail(&'static str),
    /// 会话自己停了（例如有人直接打断了整理会话）。
    Stopped,
    /// 会话那一路断了：核心在停。
    Gone,
    /// 一直等到叫停。
    Hold,
    /// 边流边交（`Watched`），交完了答这一整段：`Gate` 那里等测试放行。
    Stream(Vec<Piece>),
}

/// 流里的一样。
#[derive(Debug, Clone)]
pub enum Piece {
    Text(&'static str),
    Restart,
    /// 等测试放行（`Fake::gate`）。
    Gate,
}

/// 假的会话表：记下开的、每一批、每一次叫停。
#[derive(Debug, Default)]
pub struct Fake {
    pub opened: Mutex<Vec<DreamSpec>>,
    pub batches: Mutex<Vec<(SessionId, CommandId, String)>>,
    pub stops: Mutex<Vec<CommandId>>,
    pub steps: Mutex<VecDeque<Step>>,
    pub stopped: Notify,
    pub holding: Notify,
    pub gate: Notify,
    pub closed: Mutex<Vec<(SessionId, usize)>>,
    /// 每一次收时交来的这一趟做了什么（施工 DR-1、D-2 补）。
    pub results: Mutex<Vec<Option<DreamResult>>>,
}

pub fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn session_id() -> SessionId {
    SessionId::parse("0192f3a0-3333-7abc-8def-001122334455").expect("合写法")
}

impl DreamSessions for Fake {
    fn open(&self, spec: DreamSpec) -> Pending<'_, Result<SessionId, String>> {
        lock(&self.opened).push(spec);
        Box::pin(async { Ok(session_id()) })
    }

    fn batch<'a>(
        &'a self,
        session: SessionId,
        id: CommandId,
        text: String,
        watch: &'a mut (dyn FnMut(Watched<'_>) + Send),
    ) -> Pending<'a, Result<DreamBatch, String>> {
        lock(&self.batches).push((session, id, text));
        let step = lock(&self.steps).pop_front().expect("剧本里还有");
        Box::pin(async move {
            match step {
                Step::Stream(pieces) => {
                    let mut answer = String::new();
                    for piece in pieces {
                        match piece {
                            Piece::Text(text) => {
                                answer.push_str(text);
                                watch(Watched::Text(text));
                            }
                            Piece::Restart => {
                                answer.clear();
                                watch(Watched::Restart);
                            }
                            Piece::Gate => self.gate.notified().await,
                        }
                    }
                    Ok(DreamBatch::Answered(answer))
                }
                Step::Answer(text) => Ok(DreamBatch::Answered(text)),
                Step::Fail(why) => Ok(DreamBatch::Failed(why.to_string())),
                Step::Stopped => Ok(DreamBatch::Stopped),
                Step::Gone => Err("core stopping".to_string()),
                Step::Hold => {
                    self.holding.notify_one();
                    self.stopped.notified().await;
                    Ok(DreamBatch::Stopped)
                }
            }
        })
    }

    fn close(
        &self,
        session: SessionId,
        keep: usize,
        result: Option<DreamResult>,
    ) -> Pending<'_, Result<usize, String>> {
        lock(&self.closed).push((session, keep));
        lock(&self.results).push(result);
        Box::pin(async { Ok(0) })
    }

    fn stop(&self, _session: SessionId, id: CommandId) -> Pending<'_, Result<(), String>> {
        lock(&self.stops).push(id);
        self.stopped.notify_one();
        Box::pin(async { Ok(()) })
    }
}

/// 场地接上抽取（合并的一次性入口指着一台什么都不交的服务器：接了会话表的不该去问它），装上照 `steps` 交回的会话表。
pub async fn world(steps: Vec<Step>) -> (Home, Arc<Fake>, Server) {
    let server = Server::start(Vec::new()).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "extract_turns = 5\n");
    let fake = Arc::new(Fake {
        steps: Mutex::new(steps.into()),
        ..Fake::default()
    });
    assert!(home.memory.give_dream_sessions(fake.clone()), "头一次接上");
    (home, fake, server)
}

pub fn keeper(home: &Home) -> Keeper {
    Keeper::new(&home.memory, persona(), Listener::Owner(alice()))
}

/// 不经会话的那一趟：照场地这时的配置。
pub async fn dream(home: &Home) -> Result<Dreamed, NotDreamed> {
    let source = Arc::clone(&*home.configs.borrow());
    let config = Arc::new(Turn::new(source.with_project("~"), source));
    keeper(home)
        .dream(
            config,
            alice_account(),
            UtcOffset::from_minutes(540).expect("合写法"),
        )
        .await
}
