//! 本机才能用的那张表照参数分；远程只能往下标。

use serde_json::json;

use super::*;

fn notice(method: &str, from: &str) -> Notice {
    Notice {
        method: method.to_string(),
        params: json!({"from": from}),
    }
}

#[test]
fn some_methods_are_local_only_by_what_they_carry() {
    assert!(local_machine_only("fs.read", &json!({})));
    assert!(local_machine_only("blob.put", &json!({"path": "/a"})));
    assert!(!local_machine_only("blob.put", &json!({"data": "aGk="})));
    assert!(
        !local_machine_only("blob.put", &json!({"path": null, "data": "aGk="})),
        "null 的当没写"
    );
    assert!(local_machine_only(
        "session.create",
        &json!({"dirs": ["/a"]})
    ));
    assert!(
        !local_machine_only("session.create", &json!({"dirs": []})),
        "空的当没写"
    );
    assert!(local_machine_only(
        "session.send",
        &json!({"attachments": [{"blob": "b"}, {"path": "/a"}]})
    ));
    assert!(!local_machine_only(
        "session.send",
        &json!({"attachments": [{"blob": "b"}]})
    ));
    assert!(local_machine_only(
        "command.run",
        &json!({"text": " /workspace ~/x"})
    ));
    assert!(
        !local_machine_only("command.run", &json!({"text": "/workspace  "})),
        "只问在哪"
    );
    assert!(
        !local_machine_only("command.run", &json!({"text": "/workspaces x"})),
        "别的命令"
    );
    assert!(!local_machine_only("session.list", &json!({})));
    assert!(
        local_machine_only("schedule.add", &json!({"target": {"new": {"cwd": "/a"}}})),
        "定时任务每次新开的工作目录（施工 J-1 下）"
    );
    assert!(!local_machine_only(
        "schedule.add",
        &json!({"target": {"new": {"preset": "full"}}})
    ));
    assert!(!local_machine_only(
        "schedule.add",
        &json!({"target": {"session": "s"}})
    ));
}

#[test]
fn a_connection_is_only_ever_marked_down_and_only_before_hello() {
    let mut reach = Reach::Local;
    assert!(
        !reach.heard(&notice("connection.other", "a"), false),
        "别的通知不理"
    );
    assert!(!reach.heard(&notice(REMOTE, "a"), true), "握手以后不理");
    assert!(reach.is_local());
    assert!(reach.heard(&notice(REMOTE, "198.51.100.1"), false));
    assert_eq!(
        reach,
        Reach::Remote {
            from: "198.51.100.1".to_string()
        }
    );
    assert!(
        !reach.heard(&notice(REMOTE, "198.51.100.2"), false),
        "标过的不再改"
    );
    let mut public = Reach::Public;
    assert!(public.heard(&notice(REMOTE, "198.51.100.3"), false));
    assert!(!public.is_local(), "变不回本机");
}

#[test]
fn only_the_local_reply_keeps_its_host() {
    let shook = json!({"host": {"home": "/home/a"}, "language": "zh"});
    let mut local = shook.clone();
    strip(&Reach::Local, &mut local);
    assert_eq!(local, shook);
    let mut remote = shook.clone();
    strip(
        &Reach::Remote {
            from: String::new(),
        },
        &mut remote,
    );
    assert_eq!(remote, json!({"language": "zh"}));
}
