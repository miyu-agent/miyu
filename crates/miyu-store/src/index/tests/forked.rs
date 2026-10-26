//! fork 子代理那一行记 `forked`（施工 A-F 补）。

use super::*;

/// 从父会话第 5 条分叉的 `session.created`，`within` 照写（施工 A-F 补）：写了的是 fork 子代理，没写的是支线（C-8）。
fn forked(within: Option<u64>) -> Event {
    let within = within.map_or(String::new(), |turn| format!(r#","within":{turn}"#));
    event(&format!(
        r#"{{"seq":6,"at":"2026-09-25T07:00:00.000Z","kind":"session.created","by":{{"kind":"kernel"}},"cause":"c1","body":{{"owner":"alice","venue":"local","policy":"sha256:97f5f58cebf9e368ddcc668976ce5da07ceb80c7be52ac6d4edcf2ac8a639894","permission":{{"level":"workspace","read_only":false}},"parent":"0192f3a0-1111-7abc-8def-0123456789ab","depth":1,"forked":{{"from":"0192f3a0-1111-7abc-8def-0123456789ab","upto":5{within}}}}}}}"#
    ))
}

/// fork 子代理那一行记 `forked`（施工 A-F 补），存进表、读回来照样是；支线、普通的会话不是。
#[test]
fn only_a_fork_within_a_turn_is_marked_forked() {
    for (first, expected) in [
        (forked(Some(3)), true),
        (forked(None), false),
        (created(), false),
    ] {
        let scratch = Scratch::new();
        let (index, _) = open(&scratch);
        // 分叉出来的日志从分叉点往后数，不走 `advance` 新起：列会话时照日志造好写进去（`put`）。
        let listed = Row::new(id(), &first).unwrap();
        assert_eq!(listed.forked, expected);
        assert!(index.put(&listed, None).unwrap());
        // 重开一回：读的是表里存的。
        drop(index);
        let (index, opened) = open(&scratch);
        assert!(matches!(opened, Opened::Kept));
        assert_eq!(row(&index).unwrap().forked, expected, "存进表里的");
    }
}
