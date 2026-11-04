//! 不解认种类（施工 V-41 上，`Event::kind_written`）：内核写出去的每一行都认得出，认出来的就是读成的种类；别的样子的交回空的，
//! 调的一方去解。样本住在设计文档旁边，所以放在集成测试里读文件。

use std::path::PathBuf;

use miyu_kernel::event::Event;

const USER: &str = r#"{"seq":41,"at":"2026-09-25T07:04:05.123Z","kind":"message.user","by":{"kind":"person"},"cause":"cmd-7f3a","body":{"blocks":[{"type":"text","text":"看看 src 目录"}]}}"#;

#[test]
fn the_kind_of_a_written_line_is_read_without_parsing() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/designs/samples/events");
    let mut seen = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        for line in text.lines() {
            let event = Event::from_line(line).unwrap();
            let written = event.to_line();
            assert_eq!(
                Event::kind_written(written.as_bytes()),
                Some(event.body.kind()),
                "{written}"
            );
            seen += 1;
        }
    }
    assert!(seen > 90, "样本都在：{seen}");
    assert_eq!(Event::kind_written(USER.as_bytes()), Some("message.user"));
    // 内核写不出来的样子：认不出，交回空的。
    for other in [
        USER.replace(r#"{"seq":41,"#, r#"{ "seq":41,"#),
        USER.replace(
            r#""seq":41,"at":"2026-09-25T07:04:05.123Z","#,
            r#""at":"2026-09-25T07:04:05.123Z","seq":41,"#,
        ),
        USER.replace(r#""kind":"message.user""#, r#""kind": "message.user""#),
        USER.replace(r#""kind":"message.user""#, r#""kind":"message\u002euser""#),
        USER.replace(r#""seq":41"#, r#""seq":"41""#),
        USER.replace(r#""seq":41"#, r#""seq":"#),
        USER.replace(r#"07:04:05.123Z""#, r#"07:04:05\".123Z""#),
        r#"{"seq":41,"at":"x","kind":"message.user"#.to_string(),
        String::new(),
    ] {
        assert_eq!(Event::kind_written(other.as_bytes()), None, "{other}");
    }
    // 打头那一格之后又写了一遍 `kind` 的：读不成，认出来的不会和读成的对不上。
    let twice = USER.replace(r#""by":"#, r#""kind":"turn.started","by":"#);
    assert_eq!(Event::kind_written(twice.as_bytes()), Some("message.user"));
    let error = Event::from_line(&twice).unwrap_err().to_string();
    assert!(error.contains("duplicate field"), "{error}");
}
