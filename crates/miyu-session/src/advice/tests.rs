use miyu_kernel::id::{CallId, CommandId, SessionId};

use super::{advising, command, is_advice};

const PARENT: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";

#[test]
fn an_advice_command_is_told_apart_from_a_job() {
    let parent = SessionId::parse(PARENT).expect("测试的数据合写法");
    let call = CallId::parse("call_7_1").expect("测试的数据合写法");
    let asked = command(&parent, call);
    assert_eq!(asked.as_str(), format!("{PARENT}/advisor/call_7_1"));
    assert!(is_advice(&parent, &asked));
    // 子代理的、别的会话的、只像一半的都不算。
    let job = CommandId::parse(&format!("{PARENT}/j1")).expect("测试的数据合写法");
    assert!(!is_advice(&parent, &job));
    let other = SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d92").expect("测试的数据合写法");
    assert!(!is_advice(&other, &asked));
    let half = CommandId::parse(&format!("{PARENT}/advisory/x")).expect("测试的数据合写法");
    assert!(!is_advice(&parent, &half));
}

#[test]
fn only_the_advisor_origin_is_advising() {
    assert!(advising(Some("advisor")));
    assert!(!advising(Some("terminal")));
    assert!(!advising(None));
}
