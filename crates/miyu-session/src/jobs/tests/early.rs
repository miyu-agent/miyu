//! 起它的那次调用的结果落盘以前到的先扣下（施工 RB-1，`gate.rs`）：结束、监视攒好的行、到了上限停掉的那一条，都等那次调用的
//! `tool.result` 记下了才交给 actor，照到的先后；别的调用落了盘不算；结果先落了盘、命令才起来的不扣。

use super::*;

/// 表里 `job` 记成报过了：等的线程拿着表的锁把它记成报过，同一把锁里交出去或者扣下，看到报过时它一定已经在收件箱里、要么扣着。
/// 最多十秒。
async fn until_reported(table: &Jobs, job: u64) {
    let job = JobId::new(job).unwrap();
    for _ in 0..2000 {
        if table
            .lock()
            .iter()
            .any(|((_, id), entry)| *id == job && entry.reported)
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("十秒内 {job} 没报");
}

/// 扣下了 `n` 封，最多十秒。
async fn until_held(jobs: &SessionJobs, n: usize) {
    for _ in 0..2000 {
        if jobs.shared.gate().held.len() == n {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("十秒内没扣下 {n} 封");
}

/// 另一次调用。
fn other_call() -> CallId {
    CallId::parse("call_5_2").unwrap()
}

#[tokio::test]
async fn an_end_waits_until_the_call_that_started_it_is_recorded() {
    let scratch = Scratch::new();
    let table = Arc::new(Jobs::new());
    let (mut jobs, mut back) = session(&scratch, &table, 0);
    let _other = jobs.port(other_call(), cause(), roomy());
    let held = Held::new(&["hi\n"]);
    held.end(Exit::Code(0));
    jobs.port(call(), cause(), roomy())
        .start(held.background())
        .unwrap();
    until_reported(&table, 1).await;
    assert!(
        back.try_recv().is_err(),
        "起它的那次调用的结果还没落盘：扣着，不交"
    );
    landed(&jobs, other_call());
    assert!(back.try_recv().is_err(), "别的调用落了盘不算");
    landed(&jobs, call());
    let ended = next_end(&mut back).await;
    assert_eq!(ended.reported.job.to_string(), "j1", "落了盘就交");
    jobs.arrived(at(), ended);
    jobs.land();
    assert!(!table.running(), "交进内核、落了盘照旧从表里拿掉");
}

#[tokio::test]
async fn lines_and_a_stop_at_the_limit_wait_too_and_keep_their_order() {
    let scratch = Scratch::new();
    let table = Arc::new(Jobs::new());
    let (jobs, mut back) = session(&scratch, &table, 0);
    let held = Held::new(&["one\n"]);
    let mut background = held.background();
    background.lines = Some(Box::new(vec!["one\n".to_string()].into_iter()));
    let short = JobLimits {
        running: Duration::from_secs(1),
        ..roomy()
    };
    jobs.port(call(), cause(), short).start(background).unwrap();
    // 攒好的那一批先到，跑到最长时间停掉的那一条后到：两封都扣着。
    until_held(&jobs, 2).await;
    assert!(back.try_recv().is_err(), "行和停掉的那一条都扣着");
    landed(&jobs, call());
    let first = back.try_recv().expect("先交行");
    assert!(matches!(first, Back::JobLines(_)), "{first:?}");
    let Ok(Back::Job(ended)) = back.try_recv() else {
        panic!("再交停掉的那一条")
    };
    assert_eq!(ended.reported.reason, JobReason::TimedOut);
}

#[tokio::test]
async fn a_command_started_after_its_call_was_recorded_is_not_held() {
    let scratch = Scratch::new();
    let table = Arc::new(Jobs::new());
    let (jobs, mut back) = session(&scratch, &table, 0);
    let port = jobs.port(call(), cause(), roomy());
    // 被打断的调用：内核先替它记了结果，工具还接着跑完、起了命令。不扣：扣下的再也等不到交出去的那一刻，任务表一直不空。
    landed(&jobs, call());
    let held = Held::new(&[]);
    held.end(Exit::Code(0));
    port.start(held.background()).unwrap();
    let ended = next_end(&mut back).await;
    assert_eq!(ended.reported.job.to_string(), "j1");
}
