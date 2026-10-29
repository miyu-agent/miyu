//! 整理会话（施工 D-1 下）：`session.created` 的 `dream` 照原样记进一行，写进表里读得回来；别的会话没有。

use super::*;

#[test]
fn a_dream_session_keeps_its_room_in_the_row() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    let dreaming = event(
        r#"{"seq":1,"at":"2026-10-21T07:00:00.000Z","kind":"session.created","by":{"kind":"module","id":"memory"},"cause":"c1","body":{"venue":"local","policy":"sha256:97f5f58cebf9e368ddcc668976ce5da07ceb80c7be52ac6d4edcf2ac8a639894","permission":{"level":"workspace","read_only":true},"dream":{"persona":"engineer","manual":true}}}"#,
    );
    let mut listed = Row::new(id(), &dreaming).unwrap();
    let dream = listed.dream.clone().expect("带 dream");
    assert_eq!(dream.persona.as_deref(), Some("engineer"));
    assert!(dream.manual);
    listed.mark = mark(100, 2);
    assert!(index.put(&listed, None).unwrap());
    assert_eq!(row(&index).unwrap(), listed, "写进去读得回来");

    assert_eq!(
        Row::new(id(), &created()).unwrap().dream,
        None,
        "别的会话没有"
    );
}

/// 这一趟的结果（施工 DR-1）：照最后一条 `dream.ended` 记进一列，叫停的带 `stopped`，写进表里读得回来；没有的是空的。
#[test]
fn a_dream_session_keeps_its_last_result() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    let dreaming = event(
        r#"{"seq":1,"at":"2026-10-21T07:00:00.000Z","kind":"session.created","by":{"kind":"module","id":"memory"},"cause":"c1","body":{"venue":"local","policy":"sha256:97f5f58cebf9e368ddcc668976ce5da07ceb80c7be52ac6d4edcf2ac8a639894","permission":{"level":"workspace","read_only":true},"dream":{"persona":"engineer","manual":true}}}"#,
    );
    let mut listed = Row::new(id(), &dreaming).unwrap();
    assert_eq!(listed.dream_result, None, "还没跑完的没有");
    listed.see(&event(
        r#"{"seq":2,"at":"2026-10-21T07:01:00.000Z","kind":"dream.ended","by":{"kind":"module","id":"memory"},"cause":"c2","body":{"given":1,"revised":0,"retired":0,"summary":false}}"#,
    ));
    listed.see(&event(
        r#"{"seq":3,"at":"2026-10-21T07:02:00.000Z","kind":"dream.ended","by":{"kind":"module","id":"memory"},"cause":"c3","body":{"given":3,"revised":1,"retired":2,"summary":true,"stopped":true}}"#,
    ));
    let result = listed.dream_result.expect("记下了");
    assert_eq!(
        (result.given, result.revised, result.retired),
        (3, 1, 2),
        "照最后一条"
    );
    assert!(result.summary && result.stopped);
    listed.mark = mark(300, 4);
    assert!(index.put(&listed, None).unwrap());
    assert_eq!(row(&index).unwrap(), listed, "写进去读得回来");
}

/// 终端集成开的（施工 8-33 四补）：`session.created` 的 `origin` 进了一列，存了读回一样；别的会话没有。
#[test]
fn a_terminal_session_keeps_where_it_was_opened() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    let opened = event(
        r#"{"seq":1,"at":"2026-10-21T07:00:00.000Z","kind":"session.created","by":{"kind":"person"},"cause":"c1","body":{"venue":"local","policy":"sha256:97f5f58cebf9e368ddcc668976ce5da07ceb80c7be52ac6d4edcf2ac8a639894","permission":{"level":"workspace","read_only":true},"origin":"terminal"}}"#,
    );
    let mut listed = Row::new(id(), &opened).unwrap();
    assert_eq!(listed.origin.as_deref(), Some("terminal"));
    listed.mark = mark(100, 2);
    assert!(index.put(&listed, None).unwrap());
    assert_eq!(row(&index).unwrap(), listed, "写进去读得回来");
    assert_eq!(
        Row::new(id(), &created()).unwrap().origin,
        None,
        "别的会话没有"
    );
}
