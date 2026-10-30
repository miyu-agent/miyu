use std::io::Write;

use super::*;

#[tokio::test]
async fn each_line_the_core_writes_is_heard() {
    for (line, heard) in [
        ("ready\n", Ready::Ready),
        ("running\n", Ready::Running),
        (
            "error 找不到资源目录\n",
            Ready::Failed("找不到资源目录".to_string()),
        ),
    ] {
        let (reader, mut writer) = io::pipe().expect("开得了管道");
        writer.write_all(line.as_bytes()).expect("写得进");
        assert_eq!(hear(reader, WAIT).await.expect("听得到"), heard);
    }
}

#[tokio::test]
async fn a_core_that_leaves_without_a_word_is_silent() {
    let (reader, writer) = io::pipe().expect("开得了管道");
    drop(writer);
    let error = hear(reader, WAIT).await.expect_err("没说话");
    assert!(matches!(error, StartError::Silent), "{error:?}");
}

#[tokio::test]
async fn a_core_that_says_nothing_for_too_long_times_out() {
    let (reader, writer) = io::pipe().expect("开得了管道");
    let error = hear(reader, Duration::from_millis(100))
        .await
        .expect_err("等太久");
    assert!(matches!(error, StartError::Timeout), "{error:?}");
    drop(writer);
}

#[test]
fn a_cold_start_on_a_busy_disk_is_waited_for() {
    // 施工 CS-1：Terminal-Bench 里几个核心同时冷启动，量到过卡 24 秒，10 秒不够。
    assert!(WAIT >= Duration::from_secs(60), "{WAIT:?}");
}
