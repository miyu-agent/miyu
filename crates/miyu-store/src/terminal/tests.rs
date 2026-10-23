//! 终端会话的指针（施工 T-22）：以前只有顶上那一个的文件照样读得出、写回时不多一格；按终端的键各记一个，读回来一样；久没用的
//! 键清掉，多了丢最久没用的。

use super::*;
use crate::test_support::Scratch;

fn at(ms: i64) -> Timestamp {
    Timestamp::from_unix_millis(ms).expect("时刻写对了")
}

fn keyed(session: &str, used: i64) -> Keyed {
    Keyed {
        pointer: Pointer {
            session: Some(session.to_string()),
            at: Some(at(used)),
        },
        used: at(used),
    }
}

#[test]
fn an_old_file_reads_and_writes_back_the_same_shape() {
    let temp = Scratch::new();
    let path = temp.path().join(FILE);
    std::fs::create_dir_all(temp.path()).expect("建得了");
    std::fs::write(&path, r#"{"session":"s1","at":"2026-10-21T00:00:00.000Z"}"#).expect("写得进");
    let (read, version) = read(&path).expect("读得出");
    assert_eq!(read.main.session.as_deref(), Some("s1"));
    assert!(read.terminals.is_empty());
    write(&path, &read, version.as_deref()).expect("写得回");
    let text = std::fs::read_to_string(&path).expect("在");
    assert!(!text.contains("terminals"), "没有键的不写这一格：{text}");
}

#[test]
fn each_terminal_keeps_its_own_pointer() {
    let temp = Scratch::new();
    let path = temp.path().join(FILE);
    let mut pointers = Pointers::default();
    pointers
        .terminals
        .insert("101:5".to_string(), keyed("a", 1_000));
    pointers
        .terminals
        .insert("202:7".to_string(), keyed("b", 2_000));
    write(&path, &pointers, None).expect("写得进");
    let (again, _) = read(&path).expect("读得回");
    assert_eq!(again, pointers);
    assert_eq!(again.main.session, None, "不报键的那一个不受影响");
}

#[test]
fn stale_and_extra_terminals_are_dropped() {
    let now = FORGET_AFTER_MS + 10_000_000;
    let mut pointers = Pointers::default();
    pointers
        .terminals
        .insert("old".to_string(), keyed("x", now - FORGET_AFTER_MS - 1));
    pointers
        .terminals
        .insert("edge".to_string(), keyed("y", now - FORGET_AFTER_MS));
    pointers.prune(at(now));
    assert!(
        !pointers.terminals.contains_key("old"),
        "超过 7 天没用的清掉"
    );
    assert!(pointers.terminals.contains_key("edge"), "正好到线的留着");

    let mut many = Pointers::default();
    for n in 0..(MOST_TERMINALS as i64 + 3) {
        many.terminals
            .insert(format!("k{n}"), keyed("s", now - 1_000 + n));
    }
    many.prune(at(now));
    assert_eq!(many.terminals.len(), MOST_TERMINALS);
    for n in 0..3 {
        assert!(
            !many.terminals.contains_key(&format!("k{n}")),
            "丢最久没用的"
        );
    }
}
