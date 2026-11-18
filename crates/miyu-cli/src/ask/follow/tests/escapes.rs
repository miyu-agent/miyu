//! 模型给的字里的控制字符（审计 AUD-5）：接着终端的那一路洗掉（控制字符换成 `�`，回车去掉，换行照留），接管道的原样。

use super::*;
use crate::shown::{GRAY, RESET};

/// 思考、回答里都带着改标题、清屏、回到行首的序列；改标题那一截切在两段增量里。
fn nasty_turn() -> Vec<Value> {
    vec![
        event("turn.started", 3, "ask-4", json!({"trigger": 2})),
        delta(json!({"index": 0, "start": "reasoning"})),
        delta(json!({"index": 0, "text": "想\x1b]0;x"})),
        delta(json!({"index": 0, "text": "\x07\x1b[2J\r盖\n"})),
        delta(json!({"index": 1, "start": "text"})),
        delta(json!({"index": 1, "text": "答\x1b]0;"})),
        delta(json!({"index": 1, "text": "x\x07\x1b[2J\r伪造\n"})),
        event("turn.ended", 3, "ask-4", json!({"reason": "completed"})),
    ]
}

/// 标准错误、标准输出各接不接终端，喂完 [`nasty_turn`]，交回标准输出、标准错误。标准错误接终端的上灰色。
fn shown(err_terminal: bool, out_terminal: bool) -> (String, String) {
    let plan = plan(Output::Text, Language::Chinese);
    let tape = Tape::default();
    let (mut out, mut err) = (
        Pen {
            tape: tape.clone(),
            err: false,
        },
        Pen {
            tape: tape.clone(),
            err: true,
        },
    );
    let mut screen = Screen {
        out: &mut out,
        err: &mut err,
        gray: err_terminal,
        live: err_terminal,
        out_terminal,
    };
    let mut follow = Follow::new("s1", "ask-4", &plan);
    let messages = nasty_turn();
    let mut step = Step::Going;
    for message in &messages {
        step = follow.take(message, &mut screen);
    }
    late_reply(&mut follow, step, &messages, &mut screen);
    (tape.text(|err| !err), tape.text(|err| err))
}

/// 洗过的回答、思考。
const CLEAN_ANSWER: &str = "答\u{FFFD}]0;x\u{FFFD}\u{FFFD}[2J伪造\n";
const CLEAN_THOUGHT: &str = "想\u{FFFD}]0;x\u{FFFD}\u{FFFD}[2J盖\n";

/// 原样的回答、思考。
const RAW_ANSWER: &str = "答\x1b]0;x\x07\x1b[2J\r伪造\n";
const RAW_THOUGHT: &str = "想\x1b]0;x\x07\x1b[2J\r盖\n";

#[test]
fn control_sequences_from_the_model_are_defused_on_a_terminal() {
    let (out, err) = shown(true, true);
    assert_eq!(out, CLEAN_ANSWER);
    // 灰色是自己加的，照旧；洗的只是模型给的字。
    assert!(
        err.contains(&format!("{GRAY}想\u{FFFD}]0;x{RESET}")),
        "{err:?}"
    );
    assert!(err.contains("\u{FFFD}\u{FFFD}[2J盖\n"), "{err:?}");
    for planted in ["\x1b]0", "\x1b[2J", "\x07", "\r"] {
        assert!(!err.contains(planted), "{planted:?} 留在了 {err:?}");
    }
}

#[test]
fn control_sequences_pass_through_to_a_pipe() {
    let (out, err) = shown(false, false);
    assert_eq!(out, RAW_ANSWER);
    assert!(err.contains(RAW_THOUGHT), "{err:?}");
}

#[test]
fn each_stream_is_defused_only_when_it_is_a_terminal() {
    // 标准错误接终端、回答重定向进文件：思考洗，回答原样。
    let (out, err) = shown(true, false);
    assert_eq!(out, RAW_ANSWER);
    assert!(!err.contains("\x1b]0"), "{err:?}");
    // 反过来：回答接终端、标准错误重定向。
    let (out, err) = shown(false, true);
    assert_eq!(out, CLEAN_ANSWER);
    assert!(err.contains(RAW_THOUGHT), "{err:?}");
    assert!(!err.contains(CLEAN_THOUGHT));
}
