//! 读得慢的时候（施工 LF-6、LF-6 修，`docs/blueprint/memory.md` 第三条第 4 款、第 9 款）：出厂的时限过了这一轮不带、下一次醒来
//! 再交；测试场地平常等得很长，读比出厂的时限慢也照样带。

use super::*;

/// 测「读得慢的不挡回合」时等多久：测试场地平常等得很长（`support` 的 `Home::new`），这里换回很短的。
const SHORT: std::time::Duration = std::time::Duration::from_millis(50);

/// 读比出厂的时限还慢，这一轮照样带：测试场地等得很长，不拿真的磁盘去赛出厂的几百毫秒（施工 LF-6 修；Windows 的 CI 上同一个
/// 文件夹里两条测试各输过一回）。有人把场地的时限改回去，这一条每次都红，不是偶尔红。
#[tokio::test]
async fn a_read_slower_than_the_shipped_limit_still_arrives_in_tests() {
    let home = Home::new();
    note(&home, "主人养了一只猫");
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let handle = home.create(&script).await;
    chat(&handle, "cmd-1", "在吗").await;
    let other = elsewhere(&home).await;
    let pad = save(&home, "群友换了手柄", &[&other], alice(), None);
    home.memory
        .slow_reads(std::time::Duration::from_millis(400));
    wake(&handle, "cmd-2").await;
    home.memory.slow_reads(std::time::Duration::ZERO);
    assert_eq!(
        injected(&home, handle.id(), "fresh")
            .into_iter()
            .map(|(_, refs)| refs)
            .collect::<Vec<_>>(),
        [vec![pad.to_string()]],
        "慢了 400 毫秒，照样交了"
    );
    stop(&handle).await;
}

/// 读得慢的不挡回合（同常驻那一块，`memory.md` 第三条第 4 款）：另一个线程握着记忆日志的锁，这一轮过了时限照常请求、不带这一块；
/// 放开以后下一次醒来再问，交了。时限是几毫秒不测。
#[tokio::test]
async fn a_slow_read_does_not_hold_the_wake() {
    let home = Home::new();
    note(&home, "主人养了一只猫");
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。"), Play::Says("嗯。")]);
    let handle = home.create(&script).await;
    chat(&handle, "cmd-1", "在吗").await;
    let other = elsewhere(&home).await;
    let new = save(&home, "群友换了手柄", &[&other], alice(), None);
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let (locked, held) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel::<()>();
    let holder = std::thread::spawn(move || {
        log.book(|_| {
            locked.send(()).expect("测试在等");
            assert!(released.recv().is_err(), "测试放开时关了这一头");
        });
    });
    held.recv().expect("握住了锁");
    home.memory.wait_reads(SHORT);
    wake(&handle, "cmd-2").await;
    assert_eq!(
        blocks(&script.requests()[1].1).len(),
        1,
        "过了时限，这一轮不带"
    );
    drop(release);
    holder.join().expect("放开了");
    // 过了时限的那一轮测完了：换回场地平常的，下一次醒来不赛这 50 毫秒（Windows 的 CI 上输过）。
    home.memory.wait_reads(PATIENT);
    wake(&handle, "cmd-3").await;
    assert_eq!(
        injected(&home, handle.id(), "fresh")
            .into_iter()
            .map(|(_, refs)| refs)
            .collect::<Vec<_>>(),
        [vec![new.to_string()]],
        "下一次醒来再问，交了"
    );
    stop(&handle).await;
}
