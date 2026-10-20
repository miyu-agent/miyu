//! 界面块的内容（施工 X-15 中）：六种段的每一格、上限、字里不许有的、语言表、三种动作、底栏和首页的限制、错在哪的路径；
//! 照语言挑字。

use crate::package::WidgetPlace;
use serde_json::{Value, json};

use super::{Problem, check, check_title, localize};

/// 六种段都用上、每一格都写了的一块。
fn full() -> Value {
    json!([
        {"type": "heading", "text": "本周"},
        {"type": "text", "text": {"zh": "晴\n23°C", "en": "Sunny\n23°C"}, "tone": "accent"},
        {"type": "pairs", "items": [{"label": "湿度", "value": "40%", "tone": "dim"}]},
        {"type": "progress", "value": 0.42, "text": "索引 42%", "mark": 0.85},
        {"type": "list", "items": [
            {"title": "构建 #128", "detail": "main", "right": "3m", "state": "doing",
             "action": {"call": "build.open", "params": {"id": 128}}},
            {"title": "构建 #127", "state": "done", "action": {"copy": "build-127"}}
        ]},
        {"type": "buttons", "items": [
            {"label": "刷新", "action": {"call": "refresh"}},
            {"label": "详情", "action": {"open": "detail"}}
        ]}
    ])
}

fn problem(place: WidgetPlace, content: &Value) -> (String, Problem) {
    let bad = check(place, &["detail"], content).expect_err("该拒");
    (bad.path, bad.problem)
}

#[test]
fn every_kind_of_part_with_every_field_is_taken() {
    for place in [WidgetPlace::Sidebar, WidgetPlace::Page] {
        assert_eq!(check(place, &["detail"], &full()), Ok(()));
    }
    assert_eq!(
        check(WidgetPlace::Sidebar, &[], &json!([])),
        Ok(()),
        "空的也收"
    );
}

#[test]
fn each_mistake_says_where_and_what() {
    let cases: Vec<(Value, &str, Problem)> = vec![
        (json!({"type": "text"}), "content", Problem::Type),
        (json!(["x"]), "content[0]", Problem::Type),
        (json!([{"text": "x"}]), "content[0].type", Problem::Missing),
        (json!([{"type": "table"}]), "content[0].type", Problem::Type),
        (json!([{"type": 3}]), "content[0].type", Problem::Type),
        (
            json!([{"type": "text"}]),
            "content[0].text",
            Problem::Missing,
        ),
        (
            json!([{"type": "text", "text": "x", "color": "red"}]),
            "content[0].color",
            Problem::Unknown,
        ),
        (
            json!([{"type": "heading", "text": "a\nb"}]),
            "content[0].text",
            Problem::ControlChar,
        ),
        (
            json!([{"type": "text", "text": "\u{1b}[31mred"}]),
            "content[0].text",
            Problem::ControlChar,
        ),
        (
            json!([{"type": "text", "text": "abc\u{202E}def"}]),
            "content[0].text",
            Problem::ControlChar,
        ),
        (
            json!([{"type": "text", "text": "x", "tone": "pink"}]),
            "content[0].tone",
            Problem::Tone,
        ),
        (
            json!([{"type": "text", "text": {"fr": "x"}}]),
            "content[0].text.fr",
            Problem::Language,
        ),
        (
            json!([{"type": "text", "text": {}}]),
            "content[0].text",
            Problem::Language,
        ),
        (
            json!([{"type": "text", "text": {"zh": 3}}]),
            "content[0].text.zh",
            Problem::Type,
        ),
        (
            json!([{"type": "text", "text": 3}]),
            "content[0].text",
            Problem::Type,
        ),
        (
            json!([{"type": "heading", "text": "x".repeat(201)}]),
            "content[0].text",
            Problem::TooLong,
        ),
        (
            json!([{"type": "text", "text": "x".repeat(2001)}]),
            "content[0].text",
            Problem::TooLong,
        ),
        (
            json!([{"type": "pairs"}]),
            "content[0].items",
            Problem::Missing,
        ),
        (
            json!([{"type": "pairs", "items": {}}]),
            "content[0].items",
            Problem::Type,
        ),
        (
            json!([{"type": "pairs", "items": ["x"]}]),
            "content[0].items[0]",
            Problem::Type,
        ),
        (
            json!([{"type": "pairs", "items": [{"label": "a"}]}]),
            "content[0].items[0].value",
            Problem::Missing,
        ),
        (
            json!([{"type": "progress"}]),
            "content[0].value",
            Problem::Missing,
        ),
        (
            json!([{"type": "progress", "value": 1.5}]),
            "content[0].value",
            Problem::Value,
        ),
        (
            json!([{"type": "progress", "value": "half"}]),
            "content[0].value",
            Problem::Type,
        ),
        (
            json!([{"type": "progress", "value": 0.5, "mark": -0.1}]),
            "content[0].mark",
            Problem::Value,
        ),
        (
            json!([{"type": "list", "items": [{"detail": "x"}]}]),
            "content[0].items[0].title",
            Problem::Missing,
        ),
        (
            json!([{"type": "list", "items": [{"title": "x", "state": "waiting"}]}]),
            "content[0].items[0].state",
            Problem::State,
        ),
        (
            json!([{"type": "buttons", "items": [{"label": "x"}]}]),
            "content[0].items[0].action",
            Problem::Missing,
        ),
    ];
    for (content, path, kind) in cases {
        assert_eq!(
            problem(WidgetPlace::Sidebar, &content),
            (path.to_string(), kind),
            "{content}"
        );
    }
}

#[test]
fn too_many_and_too_big_are_refused() {
    let parts: Vec<Value> = (0..65)
        .map(|_| json!({"type": "heading", "text": "x"}))
        .collect();
    assert_eq!(
        problem(WidgetPlace::Page, &json!(parts)),
        ("content".into(), Problem::TooMany)
    );
    let pairs: Vec<Value> = (0..51)
        .map(|_| json!({"label": "a", "value": "b"}))
        .collect();
    assert_eq!(
        problem(
            WidgetPlace::Page,
            &json!([{"type": "pairs", "items": pairs}])
        ),
        ("content[0].items".into(), Problem::TooMany)
    );
    let items: Vec<Value> = (0..201).map(|_| json!({"title": "a"})).collect();
    assert_eq!(
        problem(
            WidgetPlace::Page,
            &json!([{"type": "list", "items": items}])
        ),
        ("content[0].items".into(), Problem::TooMany)
    );
    let buttons: Vec<Value> = (0..9)
        .map(|_| json!({"label": "a", "action": {"call": "x"}}))
        .collect();
    assert_eq!(
        problem(
            WidgetPlace::Page,
            &json!([{"type": "buttons", "items": buttons}])
        ),
        ("content[0].items".into(), Problem::TooMany)
    );
    // 64 段、每段 2000 个字的大块超过 64 KiB。
    let big: Vec<Value> = (0..40)
        .map(|_| json!({"type": "text", "text": "x".repeat(2000)}))
        .collect();
    assert_eq!(
        problem(WidgetPlace::Page, &json!(big)),
        ("content".into(), Problem::TooBig)
    );
    // 正好的上限都收。
    let parts: Vec<Value> = (0..64)
        .map(|_| json!({"type": "heading", "text": "x".repeat(200)}))
        .collect();
    assert_eq!(check(WidgetPlace::Page, &[], &json!(parts)), Ok(()));
}

#[test]
fn actions_are_one_of_call_open_copy() {
    let with =
        |action: Value| json!([{"type": "list", "items": [{"title": "x", "action": action}]}]);
    for good in [
        json!({"call": "refresh"}),
        json!({"call": "build.open-2_x", "params": {"id": 1}}),
        json!({"open": "detail"}),
        json!({"copy": "line one\nline two"}),
    ] {
        assert_eq!(
            check(WidgetPlace::Sidebar, &["detail"], &with(good.clone())),
            Ok(()),
            "{good}"
        );
    }
    for wrong in [
        json!("refresh"),
        json!({}),
        json!({"call": "Refresh"}),
        json!({"call": "x".repeat(65)}),
        json!({"call": "refresh", "params": [1]}),
        json!({"call": "refresh", "extra": 1}),
        json!({"open": "week"}),
        json!({"open": "detail", "params": {}}),
        json!({"copy": "x".repeat(2001)}),
        json!({"copy": "a\u{7}b"}),
        json!({"copy": 3}),
        json!({"call": "refresh", "copy": "x"}),
    ] {
        assert_eq!(
            problem(WidgetPlace::Sidebar, &with(wrong.clone())),
            ("content[0].items[0].action".into(), Problem::Action),
            "{wrong}"
        );
    }
}

#[test]
fn the_status_bar_takes_one_line_of_at_most_forty_columns() {
    let line = |text: Value| json!([{"type": "text", "text": text}]);
    assert_eq!(
        check(WidgetPlace::Status, &[], &line(json!("x".repeat(40)))),
        Ok(())
    );
    // 中文算两列：20 个字正好 40 列，21 个超了。
    assert_eq!(
        check(WidgetPlace::Status, &[], &line(json!("晴".repeat(20)))),
        Ok(())
    );
    assert_eq!(
        problem(
            WidgetPlace::Status,
            &line(json!({"en": "ok", "zh": "晴".repeat(21)}))
        ),
        ("content[0].text".into(), Problem::TooLong)
    );
    assert_eq!(
        problem(WidgetPlace::Status, &line(json!("a\nb"))),
        ("content[0].text".into(), Problem::Place)
    );
    assert_eq!(
        problem(
            WidgetPlace::Status,
            &json!([{"type": "heading", "text": "x"}])
        ),
        ("content[0]".into(), Problem::Place)
    );
    assert_eq!(
        problem(
            WidgetPlace::Status,
            &json!([{"type": "text", "text": "a"}, {"type": "text", "text": "b"}])
        ),
        ("content".into(), Problem::Place)
    );
    assert_eq!(
        check(WidgetPlace::Status, &[], &json!([])),
        Err(super::Bad {
            path: "content".into(),
            problem: Problem::Place
        })
    );
}

#[test]
fn the_home_screen_takes_at_most_eight_parts() {
    let parts = |n: usize| {
        json!(
            (0..n)
                .map(|_| json!({"type": "text", "text": "x"}))
                .collect::<Vec<_>>()
        )
    };
    assert_eq!(check(WidgetPlace::Home, &[], &parts(8)), Ok(()));
    assert_eq!(
        problem(WidgetPlace::Home, &parts(9)),
        ("content".into(), Problem::Place)
    );
    assert_eq!(
        check(WidgetPlace::Sidebar, &[], &parts(9)),
        Ok(()),
        "别的位置不限 8 段"
    );
}

#[test]
fn a_new_title_is_one_short_line() {
    assert_eq!(check_title(&json!("待办 2/5")), Ok(()));
    assert_eq!(
        check_title(&json!({"en": "Todo 2/5", "zh": "待办 2/5"})),
        Ok(())
    );
    for (wrong, kind) in [
        (json!("a\nb"), Problem::ControlChar),
        (json!("x".repeat(201)), Problem::TooLong),
        (json!(3), Problem::Type),
        (json!({"de": "x"}), Problem::Language),
    ] {
        assert_eq!(
            check_title(&wrong).map_err(|bad| bad.problem),
            Err(kind),
            "{wrong}"
        );
    }
}

#[test]
fn words_are_picked_in_the_connections_language() {
    let content = json!([
        {"type": "text", "text": {"zh": "晴", "en": "Sunny"}},
        {"type": "list", "items": [{"title": {"ja": "ビルド"}, "detail": "main",
                                    "action": {"call": "x", "params": {"zh": "不是字"}}}]},
        {"type": "pairs", "items": [{"label": {"en": "Humidity"}, "value": "40%"}]}
    ]);
    let zh = localize(&content, "zh");
    assert_eq!(zh[0]["text"], "晴");
    assert_eq!(
        zh[1]["items"][0]["title"], "ビルド",
        "没有中文的照 en、zh、ja 挑"
    );
    assert_eq!(zh[1]["items"][0]["detail"], "main");
    assert_eq!(
        zh[1]["items"][0]["action"]["params"],
        json!({"zh": "不是字"}),
        "参数不动"
    );
    assert_eq!(zh[2]["items"][0]["label"], "Humidity");
    assert_eq!(localize(&content, "ja")[0]["text"], "Sunny");
}

#[test]
fn the_right_side_of_an_item_may_be_spans_with_their_own_tone() {
    let with = |right: Value| json!([{"type": "list", "items": [{"title": "src/main.rs", "right": right}]}]);
    let spans =
        json!([{"text": "+12", "tone": "good"}, {"text": {"zh": "−3", "en": "-3"}, "tone": "bad"}]);
    assert_eq!(
        check(WidgetPlace::Sidebar, &[], &with(spans.clone())),
        Ok(())
    );
    assert_eq!(
        check(WidgetPlace::Sidebar, &[], &with(json!([{"text": "3m"}]))),
        Ok(()),
        "语气可以不写"
    );
    let at = "content[0].items[0].right";
    for (wrong, path, kind) in [
        (json!([]), at.to_string(), Problem::Missing),
        (
            json!(
                (0..5)
                    .map(|n| json!({"text": n.to_string()}))
                    .collect::<Vec<_>>()
            ),
            at.to_string(),
            Problem::TooMany,
        ),
        (json!(["+12"]), format!("{at}[0]"), Problem::Type),
        (
            json!([{"tone": "good"}]),
            format!("{at}[0].text"),
            Problem::Missing,
        ),
        (
            json!([{"text": "+12", "color": "green"}]),
            format!("{at}[0].color"),
            Problem::Unknown,
        ),
        (
            json!([{"text": "+12", "tone": "green"}]),
            format!("{at}[0].tone"),
            Problem::Tone,
        ),
        (
            json!([{"text": "a\u{7}"}]),
            format!("{at}[0].text"),
            Problem::ControlChar,
        ),
        (json!(3), at.to_string(), Problem::Type),
    ] {
        assert_eq!(
            problem(WidgetPlace::Sidebar, &with(wrong.clone())),
            (path, kind),
            "{wrong}"
        );
    }
    let picked = localize(&with(spans), "en");
    assert_eq!(
        picked[0]["items"][0]["right"],
        json!([{"text": "+12", "tone": "good"}, {"text": "-3", "tone": "bad"}])
    );
    assert_eq!(
        localize(&with(json!({"zh": "三分钟", "en": "3m"})), "zh")[0]["items"][0]["right"],
        "三分钟"
    );
}
