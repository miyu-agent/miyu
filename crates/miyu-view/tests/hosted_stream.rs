//! 供应商侧工具的流式那几种（施工 HT-3，从 `hosted.rs` 分出来：那一份到了行数上限）：没写成回复的拿掉，中途订阅补推的一串
//! 画得一样，在跑时 `doing` 是 `tool`。

use std::sync::Arc;

use miyu_kernel::accumulate::Kind;
use miyu_kernel::block::Outcome;
use miyu_kernel::event::{Body as EventBody, Piece, TransientBody};
use miyu_kernel::testkit::{Line, Streamed};
use miyu_view::{Body, Doing, Projector, ToolState};

use crate::hosted::{run, searched, start, text};
use crate::support::*;

#[test]
fn a_stream_that_never_became_a_reply_takes_the_step_away() {
    let (stage, _) = run(searched(Some(("found", false))));
    let stream: Vec<Streamed<'_>> = stage
        .stream()
        .into_iter()
        .filter(|item| !matches!(item, Streamed::Event(e) if matches!(e.body, EventBody::MessageAssistant(_))))
        .collect();
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for item in &stream {
        match item {
            Streamed::Event(event) => projector.event(event),
            Streamed::Transient(transient) => projector.transient(transient),
        };
    }
    assert!(
        of_kind(projector.entries(), "tool").is_empty(),
        "{:#?}",
        projector.entries()
    );
    assert!(
        of_kind(projector.entries(), "group").is_empty(),
        "空了的那一段跟着拿掉"
    );
}

#[test]
fn a_catch_up_burst_draws_the_same_as_watching_from_the_start() {
    let (stage, entries) = run(searched(Some(("found", false))));
    // 中途订阅补推的样子（`Session::streaming`）：这一块的增量并成开始、整段字、结果、收全四条，在它收全那一刻一起到。
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    let mut args = String::new();
    let mut burst = false;
    for item in stage.stream() {
        let transient = match item {
            Streamed::Event(event) => {
                projector.event(event);
                continue;
            }
            Streamed::Transient(transient) => transient,
        };
        let TransientBody::ModelDelta(delta) = &transient.body else {
            projector.transient(transient);
            continue;
        };
        if delta.index != 0 {
            projector.transient(transient);
            continue;
        }
        if let Piece::Text(piece) = &delta.piece {
            args.push_str(piece);
        }
        if delta.piece != Piece::End {
            continue;
        }
        let pieces = [
            Piece::Start(Kind::HostedTool {
                name: "web_search".to_string(),
            }),
            Piece::Text(args.clone()),
            Piece::Outcome(Outcome {
                text: "found".to_string(),
                failed: false,
                cut: false,
            }),
            Piece::End,
        ];
        for piece in pieces {
            let mut one = transient.clone();
            if let TransientBody::ModelDelta(d) = &mut one.body {
                d.piece = piece;
            }
            projector.transient(&one);
        }
        burst = true;
    }
    assert!(burst);
    assert_eq!(projector.entries(), entries.as_slice());
}

#[test]
fn while_it_runs_she_is_using_a_tool() {
    let mut stage = stage();
    stage.model([
        Line::streams(vec![start(0, "web_search"), text(0, r#"{"query":"rust"}"#)]).held(),
    ]);
    stage.say("搜一下");
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for item in stage.stream() {
        match item {
            Streamed::Event(event) => projector.event(event),
            Streamed::Transient(transient) => projector.transient(transient),
        };
    }
    let step = of_kind(projector.entries(), "tool")[0].id.clone();
    assert_eq!(projector.status().doing, Some(Doing::Tool { entry: step }));
    let Body::Tool(tool) = &of_kind(projector.entries(), "tool")[0].body else {
        unreachable!("是一步工具");
    };
    assert!(tool.hosted);
    assert_eq!(tool.state, ToolState::Preparing);
    assert!(tool.approval.is_none());
}
