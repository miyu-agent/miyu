//! `shell.ran` 的测试（施工 SH-1 上）：图纸上的写法读写一字不差、正常结束的不写 `reason`、不认识的 `reason` 原样留着；
//! `message.user` 的 `shell` 只在是真时写。

use super::*;
use crate::event::Body;
use crate::test_support::read_body;

#[test]
fn shell_ran_from_the_drawing_round_trips() {
    let body = r#"{"message":180,"exit":0,"duration_ms":130,"text":" M a.rs\n","omitted":0}"#;
    match read_body("shell.ran", body) {
        Body::ShellRan(ran) => assert_eq!(
            ran,
            ShellRan {
                message: Seq::new(180).unwrap(),
                exit: Some(0),
                signal: None,
                duration_ms: 130,
                text: " M a.rs\n".to_string(),
                omitted: 0,
                reason: None,
            }
        ),
        other => panic!("{other:?}"),
    }
    for reason in ["timed_out", "interrupted", "aborted", "lost_in_space"] {
        let body = format!(
            r#"{{"message":3,"signal":9,"duration_ms":0,"text":"","omitted":0,"reason":"{reason}"}}"#
        );
        match read_body("shell.ran", &body) {
            Body::ShellRan(ran) => assert_eq!(
                ran.reason.map(|r| r.as_str().to_string()),
                Some(reason.to_string())
            ),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn a_shell_message_writes_its_flag_only_when_true() {
    match read_body(
        "message.user",
        r#"{"blocks":[{"type":"text","text":"ls"}],"shell":true}"#,
    ) {
        Body::MessageUser(message) => assert!(message.shell && message.text() == "ls"),
        other => panic!("{other:?}"),
    }
    match read_body(
        "message.user",
        r#"{"blocks":[{"type":"text","text":"hi"}]}"#,
    ) {
        Body::MessageUser(message) => assert!(!message.shell),
        other => panic!("{other:?}"),
    }
}
