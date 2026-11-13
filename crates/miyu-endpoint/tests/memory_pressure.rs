//! 内存紧时把闲着的会话卸出内存（施工 MEM-2，`docs/blueprint/protocol.md`「会话表」第 10 条）：读数照测试给的
//! （`Core::with_memory_probe`），不真把内存吃满。别的程序吃满（整台机器紧、核心占得少）的照常开会话、一个都不卸；核心是大头、
//! 剩得少的，闲着的会话退下，有回合在跑的、有人订阅的不动，退下的再说话照常载入回来；cgroup 快撞上限的一样卸，大容器里还剩
//! 10% 就卸（施工 MEM-4）；开会话时叫一声就看一眼，不等隔一阵的那一次。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use miyu_endpoint::Core;
use miyu_heap::room::{Room, Rooms};
use miyu_session::testkit::{Once, Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

const GIB: u64 = 1024 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;

/// 测试里隔多久看一眼。
const QUICK: Duration = Duration::from_millis(20);

/// 整台机器只剩 3%：100 GiB 里剩 3 GiB，已用 97 GiB。
const SYSTEM_LOW: Room = Room {
    available: 3 * GIB,
    total: 100 * GIB,
};

/// 宽裕：一半。
const PLENTY: Room = Room {
    available: 50 * GIB,
    total: 100 * GIB,
};

/// 核心占了已用的一大半。
const BIG: u64 = 60 * GIB;

/// 核心才几百 MB。
const SMALL: u64 = 300 * MIB;

/// 测试中途改得了的读数：两份还剩多少、核心自己的常驻，和读了几次。
#[derive(Clone)]
struct Probe {
    now: Arc<Mutex<(Rooms, Option<u64>)>>,
    reads: Arc<AtomicUsize>,
}

impl Probe {
    fn new(system: Room, cgroup: Option<Room>, resident: u64) -> Probe {
        Probe {
            now: Arc::new(Mutex::new((
                Rooms {
                    system: Some(system),
                    cgroup,
                },
                Some(resident),
            ))),
            reads: Arc::default(),
        }
    }

    fn set(&self, system: Room, cgroup: Option<Room>, resident: u64) {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner) = (
            Rooms {
                system: Some(system),
                cgroup,
            },
            Some(resident),
        );
    }

    fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }

    /// 等到又看了 `more` 次：看完一次才读下一次，读到第 `more + 1` 次时前面那几次都判完了。
    async fn until_looked(&self, more: usize) {
        let from = self.reads();
        until("又看了几次", || self.reads() > from + more).await;
    }
}

/// 一份核心，读数照 `probe`，表里有会话时隔 `every` 看一眼；闲多久退下照出厂的三分钟，测试里等不到。
fn core(home: &Home, script: &Script, probe: &Probe, every: Duration) -> Arc<Core> {
    let (rooms, resident) = (probe.clone(), probe.clone());
    Arc::new(
        home.core_full(script, Catalog::default(), None, TOKEN)
            .with_sandbox(miyu_sandbox::Availability::Usable("miyu-sandbox".into()))
            .with_memory_probe(
                every,
                move || {
                    rooms.reads.fetch_add(1, Ordering::SeqCst);
                    rooms.now.lock().unwrap_or_else(PoisonError::into_inner).0
                },
                move || {
                    resident
                        .now
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .1
                },
            ),
    )
}

/// 等到载入着 `count` 个会话，最多六十秒。
async fn until_loaded(core: &Core, count: usize) {
    let waited = tokio::time::timeout(Duration::from_secs(60), async {
        while core.loaded().await != count {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(
        waited.is_ok(),
        "六十秒内载入着的没变成 {count} 个：{}",
        core.loaded().await
    );
}

#[tokio::test]
async fn others_eating_memory_neither_refuses_nor_retires() {
    let home = Home::new();
    let probe = Probe::new(SYSTEM_LOW, None, SMALL);
    let script = Script::new([Play::Says("好。")]).titles([Play::Says("测试")]);
    let core = core(&home, &script, &probe, QUICK);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let first = client.create("c1", "~").await;
    let said = client.say("s1", &first, "你好").await;
    assert!(said.get("result").is_some(), "{said}");
    home.until_turns(&first, 1).await;
    client.create("c2", "~").await;
    assert_eq!(home.root.sessions().expect("列得出").len(), 2, "照开");
    probe.until_looked(3).await;
    assert_eq!(core.loaded().await, 2, "一个都不卸");
}

#[tokio::test]
async fn the_core_holding_most_retires_idle_sessions_but_not_busy_or_watched_ones() {
    let home = Home::new();
    let probe = Probe::new(PLENTY, None, SMALL);
    let script = Script::new([Play::Says("好。"), Play::Holds, Play::Says("又好。")])
        .titles([Play::Says("测试"), Play::Says("测试")]);
    let core = core(&home, &script, &probe, QUICK);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let idle = client.create("c1", "~").await;
    client.say("s1", &idle, "你好").await;
    home.until_turns(&idle, 1).await;
    let busy = client.create("c2", "~").await;
    let held = client.say("s2", &busy, "慢慢来").await;
    assert!(held.get("result").is_some(), "{held}");
    let watched = client.create("c3", "~").await;
    let subscribed = client.subscribe("w1", &watched).await;
    assert!(subscribed.get("error").is_none(), "{subscribed}");
    probe.until_looked(2).await;
    assert_eq!(core.loaded().await, 3, "宽裕的不卸");
    probe.set(SYSTEM_LOW, None, BIG);
    until_loaded(&core, 2).await;
    probe.until_looked(3).await;
    assert_eq!(core.loaded().await, 2, "有回合的、有人订阅的不动");
    // 退下的再说话照常载入回来、接着上一轮。
    let said = client.say("s3", &idle, "还在吗").await;
    assert!(said.get("error").is_none(), "{said}");
    home.until_turns(&idle, 2).await;
}

#[tokio::test]
async fn a_cgroup_near_its_limit_retires_idle_sessions() {
    let home = Home::new();
    let roomy = Room {
        available: 9 * GIB,
        total: 18 * GIB,
    };
    let probe = Probe::new(PLENTY, Some(roomy), SMALL);
    let script = Script::new([Play::Says("好。")]).titles([Play::Says("测试")]);
    let core = core(&home, &script, &probe, QUICK);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "你好").await;
    home.until_turns(&session, 1).await;
    probe.until_looked(2).await;
    assert_eq!(core.loaded().await, 1);
    // bm 的样子：限 18 GiB、只剩 0.5 GiB，核心自己占得再少也卸。
    let near = Room {
        available: GIB / 2,
        total: 18 * GIB,
    };
    probe.set(PLENTY, Some(near), SMALL);
    until_loaded(&core, 0).await;
}

/// 大容器（施工 MEM-4）：限 18 GiB 还剩 1.7 GiB（9%），照 5% 不算紧，照「10%、最多 2 GiB」算紧，闲着的卸掉。bm 复测
/// （限 18 GiB）照 5% 判，判成紧时只剩 876 MiB、卸那一批卸了 43 秒。
#[tokio::test]
async fn a_large_cgroup_is_tight_with_a_tenth_left() {
    let home = Home::new();
    let probe = Probe::new(PLENTY, None, SMALL);
    let script = Script::new([Play::Says("好。")]).titles([Play::Says("测试")]);
    let core = core(&home, &script, &probe, QUICK);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "你好").await;
    home.until_turns(&session, 1).await;
    probe.until_looked(2).await;
    assert_eq!(core.loaded().await, 1);
    let tenth = Room {
        available: 17 * GIB / 10,
        total: 18 * GIB,
    };
    probe.set(PLENTY, Some(tenth), SMALL);
    until_loaded(&core, 0).await;
}

/// 隔一小时才看一次：开会话时叫的那一声当场看。
#[tokio::test]
async fn opening_a_session_looks_at_once() {
    let home = Home::new();
    let probe = Probe::new(PLENTY, None, SMALL);
    let core = core(&home, &Script::new([]), &probe, Duration::from_secs(3600));
    let mut client = Client::connect(core.clone());
    client.hello().await;
    client.create("c1", "~").await;
    until("看过一次", || probe.reads() >= 1).await;
    assert_eq!(core.loaded().await, 1);
    probe.set(SYSTEM_LOW, None, BIG);
    client.create("c2", "~").await;
    until_loaded(&core, 0).await;
    assert_eq!(home.root.sessions().expect("列得出").len(), 2, "卸的不是删");
}

/// bm 复测的样子（施工 MEM-3）：开着记忆的会话一个个订阅、说一轮、断开，每一个都上着 30 分钟的抽取闹钟。判成紧以后都卸掉，
/// 有回合在跑的、还订阅着的留着；卸了的再说话照常载入回来。改之前闹钟挡着，一个都卸不动。
#[tokio::test]
async fn released_sessions_with_memory_retire_when_tight() {
    let home = Home::new();
    let probe = Probe::new(PLENTY, None, SMALL);
    let mut plays = vec![Play::Says("好。"); 4];
    plays.push(Play::Holds);
    plays.push(Play::Says("又好。"));
    let script = Script::new(plays)
        .titles(vec![Play::Says("测试"); 6])
        .once(Vec::<Once>::new());
    let core = core(&home, &script, &probe, QUICK);
    let mut released = Vec::new();
    for n in 0..3 {
        let mut client = Client::connect(core.clone());
        client.hello().await;
        let session = client.create_with(&format!("c{n}"), remembering()).await;
        client.subscribe(&format!("w{n}"), &session).await;
        client.say(&format!("s{n}"), &session, "你好").await;
        home.until_turns(&session, 1).await;
        drop(client);
        released.push(session);
    }
    let mut watching = Client::connect(core.clone());
    watching.hello().await;
    let watched = watching.create_with("c3", remembering()).await;
    watching.subscribe("w3", &watched).await;
    watching.say("s3", &watched, "你好").await;
    home.until_turns(&watched, 1).await;
    let busy = watching.create_with("c4", remembering()).await;
    let held = watching.say("s4", &busy, "慢慢来").await;
    assert!(held.get("result").is_some(), "{held}");
    probe.until_looked(2).await;
    assert_eq!(core.loaded().await, 5, "宽裕的不卸");
    probe.set(SYSTEM_LOW, None, BIG);
    until_loaded(&core, 2).await;
    probe.until_looked(3).await;
    assert_eq!(core.loaded().await, 2, "有回合的、还订阅着的不动");
    let said = watching.say("s5", &released[0], "还在吗").await;
    assert!(said.get("error").is_none(), "{said}");
    home.until_turns(&released[0], 2).await;
}

/// 开着记忆的会话：工程师人格、全套预设（剧本有一次性入口，核心交得出抽取要的几样）。
fn remembering() -> serde_json::Value {
    serde_json::json!({"cwd": "~", "persona": "engineer", "preset": "full"})
}

/// 卸一大批的时候别的请求插得进来（施工 MEM-5）：200 个闲着的会话判成紧全卸，运行时被几个任务轮流占着（测试的运行时只有
/// 一个线程，每个任务占 1 毫秒再让出来，像 bm 复测里 939 个回合同时在跑：问一个会话要排一圈队）。卸的期间每 10 毫秒拿一次
/// 表的锁（`Core::loaded`），最久等不过 1 秒。改之前卸完整批才放锁（一个个问，一个约两圈队），要等好几秒。
#[tokio::test]
async fn others_get_the_table_while_a_large_batch_retires() {
    const MANY: usize = 200;
    let home = Home::new();
    let probe = Probe::new(PLENTY, None, SMALL);
    let core = core(&home, &Script::new([]), &probe, QUICK);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    for n in 0..MANY {
        client.create(&format!("c{n}"), "~").await;
    }
    drop(client);
    assert_eq!(core.loaded().await, MANY);
    let crowd: Vec<_> = (0..8)
        .map(|_| {
            tokio::spawn(async {
                loop {
                    std::thread::sleep(Duration::from_millis(1));
                    queue_again().await;
                }
            })
        })
        .collect();
    probe.set(SYSTEM_LOW, None, BIG);
    let (started, mut longest) = (std::time::Instant::now(), Duration::ZERO);
    loop {
        let asked = std::time::Instant::now();
        let loaded = core.loaded().await;
        longest = longest.max(asked.elapsed());
        if loaded == 0 {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "六十秒内没卸完：还载入着 {loaded} 个"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    for hog in crowd {
        hog.abort();
    }
    assert!(
        longest <= Duration::from_secs(1),
        "卸的时候拿表的锁最久等了 {longest:?}（卸完用了 {:?}）",
        started.elapsed()
    );
}

/// 让出线程、排回运行队列的末尾（像被 I/O 叫醒的回合）。`tokio::task::yield_now` 不行：它让出的任务要等队列空了、查过一遍
/// I/O 才再跑，占不住别人的队。
async fn queue_again() {
    let mut queued = false;
    std::future::poll_fn(|cx| {
        if queued {
            std::task::Poll::Ready(())
        } else {
            queued = true;
            cx.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    })
    .await;
}
