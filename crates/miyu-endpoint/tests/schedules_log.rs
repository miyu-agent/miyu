//! 写不进下一次的先放一放（施工 J-1 上，`docs/blueprint/schedules.md` 第三节第 3 条）：唤醒表被锁住、写不进的时候，到点的那一个
//! 不交，也不空转：头一次放 1 秒，再写不进翻倍；拨过十秒只试几次，每次记一行 `WARN schedule not advanced`。锁一放开，下一次照常
//! 交。手拨的钟；锁是测试自己开的一个连接拿着排他锁。
//!
//! 全局装一个写进内存的订阅者：全局的一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。

mod support;

use std::sync::Arc;
use std::time::Duration;

use miyu_endpoint::schedules::{Creator, Target, Wanted, When};
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::By;
use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};

use support::clock::{Hand, core_with};
use support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

#[tokio::test]
async fn a_locked_table_backs_off_instead_of_spinning() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let tries = || {
        memory
            .lines()
            .iter()
            .filter(|line| line.contains("schedule not advanced"))
            .count()
    };
    let home = Home::new();
    let script = Script::new([Play::Says("看过了。")]);
    let hand = Hand::at(START);
    let core = core_with(&home, &script, &hand);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &cwd).await;
    core.start_schedules();
    core.schedules()
        .add(
            &alice(),
            Wanted {
                by: Creator::Person { account: alice() },
                when: When::Every { every: 60 },
                prompt: "看看 CI".to_string(),
                target: Target::Session {
                    session: SessionId::parse(&session).expect("会话编号合写法"),
                },
                looping: None,
                first: None,
            },
        )
        .expect("建得了");
    let file = miyu_store::schedules::path(&home.root, &alice());
    let lock = rusqlite::Connection::open(&file).expect("开得了");
    lock.execute_batch("BEGIN EXCLUSIVE").expect("锁得住");

    hand.advance(60_000);
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(30)).await;
        hand.advance(1_000);
    }
    tokio::time::sleep(Duration::from_millis(30)).await;
    let tried = tries();
    assert!((1..10).contains(&tried), "十秒里只试几次：{tried}");
    let said = |log: &[miyu_kernel::event::Event]| {
        log.iter()
            .filter(|event| matches!(event.by, By::Schedule(_)))
            .count()
    };
    assert_eq!(said(&home.log(&session)), 0, "写不进下一次的不交");

    lock.execute_batch("COMMIT").expect("放得开");
    hand.advance(300_000);
    home.until_turns(&session, 1).await;
    assert_eq!(said(&home.log(&session)), 1, "放开以后照常交");
    drop(lock);
}
