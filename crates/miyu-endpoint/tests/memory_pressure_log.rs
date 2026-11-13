//! 内存紧时记的那一行（施工 MEM-3，`docs/blueprint/protocol.md`「会话表」第 10 条「记什么」）：判成紧、一个都卸不动的也记一行
//! `INFO memory tight`，写着卸了几个、问了几个、没卸的照原因各几个、照什么门槛判的（施工 MEM-4）、卸这一批花了多久（施工 MEM-5）；同一段紧着、还是没得卸的不每次都记；又卸了会话的当场再记。
//!
//! 全局装一个写进内存的订阅者：看的任务在别的线程上。全局的一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use miyu_heap::room::{Room, Rooms};
use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use support::*;

const GIB: u64 = 1024 * 1024 * 1024;

#[tokio::test]
async fn tight_with_nothing_to_retire_is_logged_once_and_again_when_some_retire() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let lines = || -> Vec<String> {
        memory
            .lines()
            .into_iter()
            .filter(|line| line.contains("memory tight"))
            .collect()
    };
    let home = Home::new();
    let (tight, reads) = (
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicUsize::new(0)),
    );
    let (reading, counting) = (Arc::clone(&tight), Arc::clone(&reads));
    let script = Script::new([Play::Holds, Play::Says("好。")]).titles([Play::Says("测试")]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_sandbox(miyu_sandbox::Availability::Usable("miyu-sandbox".into()))
            .with_memory_probe(
                Duration::from_millis(20),
                move || {
                    counting.fetch_add(1, Ordering::SeqCst);
                    let available = match reading.load(Ordering::SeqCst) {
                        true => GIB / 2,
                        false => 9 * GIB,
                    };
                    Rooms {
                        system: None,
                        cgroup: Some(Room {
                            available,
                            total: 18 * GIB,
                        }),
                    }
                },
                || None,
            ),
    );
    let looked = |more: usize| {
        let reads = Arc::clone(&reads);
        let from = reads.load(Ordering::SeqCst);
        async move {
            until("又看了几次", || {
                reads.load(Ordering::SeqCst) > from + more
            })
            .await
        }
    };
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let busy = client.create("c1", "~").await;
    let held = client.say("s1", &busy, "慢慢来").await;
    assert!(held.get("result").is_some(), "{held}");
    let watched = client.create("c2", "~").await;
    client.subscribe("w1", &watched).await;
    looked(2).await;
    assert!(lines().is_empty(), "不紧不记：{:?}", lines());

    // 紧了，一个有回合在跑、一个有人订阅：一个都卸不动，也记一行，原因写清；接着看几次不再记。
    tight.store(true, Ordering::SeqCst);
    looked(5).await;
    let first = lines();
    assert_eq!(first.len(), 1, "{first:?}");
    assert!(
        first[0].contains("INFO  endpoint memory tight retired=0 asked=0 held=\"turn=1 watched=1\" loaded=2 available_mib=512 percent=2 threshold_mib=1843"),
        "{first:?}"
    );
    // 卸这一批花了多久（施工 MEM-5）：毫秒，跟在最后。
    assert!(
        first[0].contains(" threshold_mib=1843 took_ms="),
        "{first:?}"
    );

    // 又来一个闲着的：开会话叫的那一声当场看、卸掉它，当场再记一行。
    let idle = client.create("c3", "~").await;
    client.say("s3", &idle, "你好").await;
    home.until_turns(&idle, 1).await;
    until("卸了的那一行", || {
        lines().iter().any(|line| line.contains("retired=1"))
    })
    .await;
    let again = lines();
    assert!(
        again.iter().any(|line| line
            .contains("memory tight retired=1 asked=1 held=\"turn=1 watched=1\" loaded=2")),
        "{again:?}"
    );
}
