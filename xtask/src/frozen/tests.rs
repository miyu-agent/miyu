//! 协议 1 冻结的快照（施工 Y-5 末下）：对得上的没问题；代码里没了的、新加没登记的、没照字母排的各报一条；说明行不算方法。

use super::*;

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

fn list(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

#[test]
fn a_matching_snapshot_has_no_problems() {
    let code = set(&["session.create", "tool.call"]);
    assert!(problems(&list(&["session.create", "tool.call"]), &code).is_empty());
    assert_eq!(
        listed("# 说明\n\nsession.create\n tool.call \n"),
        list(&["session.create", "tool.call"])
    );
}

#[test]
fn removed_added_and_unsorted_are_reported() {
    let code = set(&["session.create", "session.fork"]);
    let found = problems(&list(&["session.create", "session.delete"]), &code);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found[0].contains("冻结了 session.delete"), "{found:?}");
    assert!(found[1].contains("新加了 session.fork"), "{found:?}");
    let unsorted = problems(&list(&["session.fork", "session.create"]), &code);
    assert!(unsorted[0].contains("照字母排"), "{unsorted:?}");
}
