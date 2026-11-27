//! 停会话不等正在造的会话（施工 CS-9）：造会话卡在写盘的哪一步，停会话都不等它手里的表的锁，造会话的头收到 `session_stopped`；
//! 不留半个会话：建好目录、还没写 `session.created` 的删掉目录；在写的等它写完、停下，是一个载入得了的完整会话。
//!
//! 写盘慢的造法：给核心的模型端口卡住一步。`ready`（等模型目录）在建好目录以后、写 `session.created` 以前；`port`（造会话的
//! 端口）在那之后、起 actor 以前，是同步的，卡住的是造会话那个任务所在的线程。

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use miyu_kernel::event::Body;
use miyu_kernel::id::CommandId;
use miyu_session::testkit::Script;
use miyu_session::{ForSession, ModelPort, Models};
use miyu_store::log::first_event;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use super::super::Opening;
use crate::Core;
use crate::refusal::Refusal;
use crate::test_support::{TempDir, resources, temp_root};

/// 卡在哪一步。
enum Hold {
    /// 模型目录一直没备好。
    Ready,
    /// 造端口那一步等放行。
    Port(Arc<Gate>),
}

/// 造端口那一步的闸：到了没有、放没放行。
#[derive(Default)]
struct Gate {
    state: Mutex<(bool, bool)>,
    turned: Condvar,
}

impl Gate {
    /// 到了这一步，等放行。
    fn pass(&self) {
        let mut state = self.state.lock().expect("没崩");
        state.0 = true;
        while !state.1 {
            state = self.turned.wait(state).expect("没崩");
        }
    }

    fn reached(&self) -> bool {
        self.state.lock().expect("没崩").0
    }

    fn open(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.1 = true;
        self.turned.notify_all();
    }
}

/// 丢掉时放行：断言没过、测试中途 panic 的也放开卡在闸上的那个工作线程。不放的话放下运行时要等它，测试一直挂着
/// （2026-11-27 去掉修的那一处验改前红时挂了 50 分钟）。
struct Opens(Arc<Gate>);

impl Drop for Opens {
    fn drop(&mut self) {
        self.0.open();
    }
}

/// 照剧本回的端口，卡住一步。
struct Held {
    script: Script,
    hold: Hold,
}

impl Models for Held {
    fn ready(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        match self.hold {
            Hold::Ready => Box::pin(std::future::pending()),
            Hold::Port(_) => Box::pin(std::future::ready(())),
        }
    }

    fn port(&self, session: ForSession) -> Arc<dyn ModelPort> {
        if let Hold::Port(gate) = &self.hold {
            gate.pass();
        }
        self.script.port(session)
    }
}

/// 卡住 `hold` 的核心，和它的临时数据根。
fn core(name: &str, hold: Hold) -> (Arc<Core>, TempDir) {
    let (root, dir) = temp_root(name);
    let models = Held {
        script: Script::new([]),
        hold,
    };
    let core = Core::new(
        root,
        ResourceRoot::at(resources()),
        Arc::new(models),
        Catalog::default(),
        None,
        "token".to_string(),
    );
    (Arc::new(core), dir)
}

/// 在后台造一个会话（命令编号 `c1`，无人格）。
fn creating(
    core: &Arc<Core>,
    dir: &TempDir,
) -> tokio::task::JoinHandle<Result<super::super::Created, Refusal>> {
    let (core, cwd) = (
        Arc::clone(core),
        dir.as_ref().to_string_lossy().into_owned(),
    );
    tokio::spawn(async move {
        let command = CommandId::parse("c1").expect("合写法");
        core.sessions
            .create(&core, command, Some(None), cwd, Vec::new(), opening())
            .await
    })
}

fn opening() -> Opening {
    Opening {
        attended: false,
        oneshot: false,
        model: None,
        venue: None,
        memory: None,
        preset: Some(String::new()),
        group: None,
        profile: false,
        images: true,
        titles: true,
        origin: None,
        confined: false,
        voice_max_ms: None,
    }
}

/// 等到会话目录建出来（最多 30 秒）。
async fn until_a_dir(core: &Core) {
    let made = tokio::time::timeout(Duration::from_secs(30), async {
        while core.root.sessions().unwrap_or_default().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(made.is_ok(), "会话目录建出来了");
}

/// 建好目录、等模型目录时要停（修之前停会话一直等这把锁）：停会话当场做完，造会话的头收到 `session_stopped`，目录删掉了，
/// 之后再造的也是 `session_stopped`。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stopping_does_not_wait_for_a_session_being_created() {
    let (core, dir) = core("stop-creating", Hold::Ready);
    let created = creating(&core, &dir);
    until_a_dir(&core).await;
    let stopped = tokio::time::timeout(Duration::from_secs(10), core.sessions.stop_all()).await;
    assert!(stopped.is_ok(), "停会话不等正在造的会话");
    let created = tokio::time::timeout(Duration::from_secs(10), created).await;
    let created = created.expect("造会话的头收到了回应").expect("没 panic");
    assert_eq!(created.map(|made| made.id), Err(Refusal::STOPPED));
    assert!(
        core.root.sessions().expect("读得了").is_empty(),
        "不留半个会话"
    );
    let again = creating(&core, &dir).await.expect("没 panic");
    assert_eq!(
        again.map(|made| made.id),
        Err(Refusal::STOPPED),
        "停了以后不再造"
    );
}

/// 起 actor 以前要停：造会话的头当场收到 `session_stopped`、表的锁放开了（问忙不忙当场有答）；停会话等它写完
/// `session.created`、把它停下，留下的是一个载入得了的完整会话。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_session_being_written_is_finished_and_stopped_whole() {
    let gate = Arc::new(Gate::default());
    let _opens = Opens(Arc::clone(&gate));
    let (core, dir) = core("stop-writing", Hold::Port(Arc::clone(&gate)));
    let created = creating(&core, &dir);
    let reached = tokio::time::timeout(Duration::from_secs(30), async {
        while !gate.reached() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(reached.is_ok(), "造会话那一趟到了造端口那一步");
    let stopping = tokio::spawn({
        let core = Arc::clone(&core);
        async move { core.sessions.stop_all().await }
    });
    let created = tokio::time::timeout(Duration::from_secs(10), created).await;
    let created = created
        .expect("造会话的头当场收到了回应")
        .expect("没 panic");
    assert_eq!(created.map(|made| made.id), Err(Refusal::STOPPED));
    let asked = tokio::time::timeout(Duration::from_secs(10), core.sessions.busy()).await;
    assert_eq!(asked, Ok(false), "表的锁放开了");
    assert!(!stopping.is_finished(), "停会话等着在写的那一个");
    gate.open();
    let stopped = tokio::time::timeout(Duration::from_secs(30), stopping).await;
    assert!(matches!(stopped, Ok(Ok(()))), "写完、停下了");
    let ids = core.root.sessions().expect("读得了");
    assert_eq!(ids.len(), 1, "留下一个会话");
    let first = first_event(&core.root.session_dir(&ids[0])).expect("读得到第一条");
    assert!(
        matches!(first.body, Body::SessionCreated(_)),
        "是完整的会话"
    );
    assert!(
        core.sessions.loaded(&ids[0]).await.is_none(),
        "停下了、不在表里"
    );
}
