//! 整理会话（施工 D-1 下）：`session.created` 的 `dream` 照原样记进一行，写进表里读得回来；别的会话没有。

use super::*;

#[test]
fn a_dream_session_keeps_its_room_in_the_row() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    let dreaming = event(
        r#"{"seq":1,"at":"2026-10-21T07:00:00.000Z","kind":"session.created","by":{"kind":"module","id":"memory"},"cause":"c1","body":{"owner":"alice","venue":"local","policy":"sha256:97f5f58cebf9e368ddcc668976ce5da07ceb80c7be52ac6d4edcf2ac8a639894","permission":{"level":"workspace","read_only":true},"dream":{"persona":"engineer","manual":true}}}"#,
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
