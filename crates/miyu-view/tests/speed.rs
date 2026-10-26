//! 一轮收尾的出字速度（施工 SP-1）：每次主请求的输出 token、首字到结束的毫秒各自加起来，照会话状态的 `speed` 同一个口径；
//! 调工具、等人、旁路请求不算，没报首字的照整次，以前的日志没记用时的不写。日志照脚本模型跑出来，喂给投影之前改写每次请求记下的数。

use std::cell::Cell;
use std::sync::Arc;

use miyu_kernel::event::{Body as EventBody, Event, ModelCalled, Purpose, Usage};
use miyu_kernel::testkit::{Line, Play, Stage};
use miyu_kernel::time::Timestamp;
use miyu_view::{Projector, Speed};

use crate::support::*;

/// 她先调一次工具、再说话：一轮里两次主请求。
fn two_calls() -> Stage {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[("read", r#"{"file_path":"/home/alice/src/a.rs"}"#)]),
        Line::says("Done."),
    ]);
    stage.tools([Play::done("fn main() {}")]);
    stage.say("read a");
    stage
}

/// 第几次主请求记下的输出、首字、整次：`None` 的格照没记。
type Took = (Option<u64>, Option<u64>, Option<u64>);

/// 照 `took` 改写每次主请求，`more` 再改别的事件，喂给新的投影。
fn project(stage: &Stage, took: &[Took], more: impl Fn(&mut Event)) -> Projector {
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    let mut n = 0;
    for event in stage.log() {
        let mut event = event.clone();
        if let EventBody::ModelCalled(called) = &mut event.body {
            let (output, first, whole) = took[n];
            n += 1;
            rewrite(called, output, first, whole);
        }
        more(&mut event);
        projector.event(&event);
    }
    assert_eq!(n, took.len(), "主请求的次数对得上");
    projector
}

fn rewrite(called: &mut ModelCalled, output: Option<u64>, first: Option<u64>, whole: Option<u64>) {
    match output {
        Some(output) => {
            called.usage = Some(Usage {
                uncached: 0,
                cache_read: 0,
                cache_write: 0,
                output,
                reasoning: None,
            });
        }
        None => called.usage = None,
    }
    called.first_token_ms = first;
    called.duration_ms = whole;
}

fn end(projector: &Projector) -> serde_json::Value {
    json(of_kind(projector.entries(), "end")[0])
}

#[test]
fn a_turns_speed_adds_up_every_main_request() {
    let stage = two_calls();
    let projector = project(
        &stage,
        &[
            (Some(100), Some(500), Some(2_000)),
            (Some(50), Some(200), Some(1_000)),
        ],
        |_| {},
    );
    let end = end(&projector);
    assert_eq!(end["speed"]["output"], 150, "{end}");
    assert_eq!(
        end["speed"]["ms"],
        1_500 + 800,
        "首字到结束各自加起来：{end}"
    );
    assert_eq!(
        projector.status().speed,
        Some(Speed {
            output: 50,
            ms: 800
        }),
        "会话状态照旧是最近一次"
    );
}

#[test]
fn tool_time_between_the_requests_does_not_count() {
    let stage = two_calls();
    // 工具跑了一分钟：第一次请求之后的事件都晚一分钟到，一轮的用时变长，速度不变。
    let after_first = Cell::new(false);
    let late = |event: &mut Event| {
        if after_first.get() {
            event.at =
                Timestamp::from_unix_millis(event.at.unix_millis() + 60_000).expect("时刻在范围里");
        }
        if matches!(event.body, EventBody::ModelCalled(_)) {
            after_first.set(true);
        }
    };
    let projector = project(
        &stage,
        &[
            (Some(100), Some(500), Some(2_000)),
            (Some(50), Some(200), Some(1_000)),
        ],
        late,
    );
    let end = end(&projector);
    assert!(
        end["took_ms"].as_u64().unwrap_or_default() >= 60_000,
        "{end}"
    );
    assert_eq!(end["speed"]["ms"], 2_300, "{end}");
}

#[test]
fn a_side_request_in_the_turn_is_left_out() {
    let stage = two_calls();
    let took = [
        (Some(100), Some(500), Some(2_000)),
        (Some(50), Some(200), Some(1_000)),
    ];
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    let mut n = 0;
    for event in stage.log() {
        let mut event = event.clone();
        if let EventBody::ModelCalled(called) = &mut event.body {
            let (output, first, whole) = took[n];
            n += 1;
            rewrite(called, output, first, whole);
            // 紧跟着一次起标题的旁路请求，数大得显眼。
            let mut aside = event.clone();
            if let EventBody::ModelCalled(called) = &mut aside.body {
                called.purpose = Some(Purpose::Title);
                rewrite(called, Some(9_000), None, Some(9_000));
            }
            projector.event(&event);
            projector.event(&aside);
            continue;
        }
        projector.event(&event);
    }
    let end = end(&projector);
    assert_eq!(end["speed"]["output"], 150, "{end}");
    assert_eq!(end["speed"]["ms"], 2_300, "{end}");
    assert_eq!(
        projector.status().speed,
        Some(Speed {
            output: 50,
            ms: 800
        }),
        "会话状态也不算旁路"
    );
}

#[test]
fn a_request_without_a_first_token_counts_whole() {
    let stage = two_calls();
    let projector = project(
        &stage,
        &[
            (Some(100), None, Some(2_000)),
            (Some(50), Some(200), Some(1_000)),
        ],
        |_| {},
    );
    let end = end(&projector);
    assert_eq!(end["speed"]["ms"], 2_000 + 800, "{end}");
}

#[test]
fn an_old_log_without_timings_writes_no_speed() {
    let stage = two_calls();
    let projector = project(
        &stage,
        &[(Some(100), None, None), (Some(50), None, None)],
        |_| {},
    );
    let end = end(&projector);
    assert!(end.get("speed").is_none(), "没记用时的不写：{end}");
    assert_eq!(end["usage"]["output"], 150, "用量照旧加：{end}");
    assert_eq!(projector.status().speed, None);
    // 只有一次记了用时：照记下的那一次算，没记的不掺进来。
    let half = project(
        &stage,
        &[(Some(100), None, None), (Some(50), Some(200), Some(1_000))],
        |_| {},
    );
    assert_eq!(end_speed(&half), Some((50, 800)));
}

fn end_speed(projector: &Projector) -> Option<(u64, u64)> {
    let end = end(projector);
    Some((
        end["speed"]["output"].as_u64()?,
        end["speed"]["ms"].as_u64()?,
    ))
}
