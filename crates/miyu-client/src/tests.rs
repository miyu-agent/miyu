//! 门面上的每一样都在（设计 32 第二节第 2 条）：照两个头 2026-10-11 报的用法、接入QQ 和群聊内核 2026-10-16 数的（施工 S-6 上）
//! 列一遍，编得过就是在。改了、少了门面上的东西，这里跟着改，照改协议的规矩告诉两个头。

#[expect(unused_imports, reason = "只为钉住门面：拿掉了哪一样，这里当场编不过")]
use crate::{
    CoreCommand,
    connect::{
        ConnectError, Connection, Ready, StartError, connect, connect_or_start,
        connect_or_start_bare, spawn_detached,
    },
    // 施工 S-6 上：接入QQ 和群聊内核用到的几组。
    events::{Block, Body, By, Event, FormatError},
    heap::{Stats, Usage, resident, stats, trim, tune, usage},
    ids::{ContentHash, ExternalId, Seq, VenueId},
    log::{LevelFilter, install},
    manifest::{Value, read},
    open::{Browser, Core, SystemBrowser},
    places::{
        DataRoot, Env, Issue, Packages, Platform, ResourceRoot, cache_root, config_file, locale,
        write,
    },
    protocol::{Said, SessionId, Template},
    seeds::{MAX, Seed, from_image, from_rgba},
    settings::{
        Kind, Secret,
        parse::{read as read_value, why},
        problem::{At, Code, Severity, got, nearest, tell},
    },
    texts::{Face, Human, HumanError, clean},
    time::{Timestamp, UtcOffset},
    // 施工 X-28：接入QQ 查插件给的界面块。
    widgets::{Bad, Problem, WidgetPlace, check, check_title, localize, pick},
};

#[cfg(feature = "http")]
#[expect(unused_imports, reason = "只为钉住门面：拿掉了哪一样，这里当场编不过")]
use crate::http::{Get, Got, Proxy, fetcher, get, is_loopback_url};

#[cfg(feature = "tool")]
#[cfg(feature = "view")]
#[expect(unused_imports, reason = "只为钉住门面：拿掉了哪一样，这里当场编不过")]
use crate::view::{
    Answered, Approval, Attachment, Body as ViewBody, Change, Compaction, CompactionState, Diff,
    Doing, End, Entry, EntryId, Group, JobDone, JobEnd, JobRow, JobState, Mark, Media, ModelSwap,
    Nested, NestedCall, NestedItem, Notice, Output, Part, Peer, Picture, Preparing, Recast, Reply,
    SkillUsed, Speed, State, Status, Stored, Swapped, Thought, Title, Tone, Tool, ToolState,
    TouchKind, Touched, User, VERSION, Wait, Waiting, Was,
};

#[cfg(feature = "wallpaper")]
#[expect(unused_imports, reason = "只为钉住门面：拿掉了哪一样，这里当场编不过")]
use crate::wallpaper::{
    MAX_BYTES, TIMEOUT, Wallpaper, find,
    matugen::{self, Matugen},
    watch::{Watch, files_of, watch_list},
};

#[expect(unused_imports, reason = "只为钉住门面：拿掉了哪一样，这里当场编不过")]
use crate::tooltext::{
    Access, Spec,
    load::{LoadError, spec, spec_in, text, text_in},
};

/// 界面用不上的两组放在 feature 后面、默认不开：`miyu-http`、`miyu-tool` 是可选的依赖，不在默认的 feature 里；只有头要的
/// `miyu-wallpaper` 同样（施工 X-29）。
#[test]
fn the_heavy_groups_are_off_by_default() {
    let manifest: toml_edit::DocumentMut =
        include_str!("../Cargo.toml").parse().expect("清单读得懂");
    for dep in ["miyu-http", "miyu-tool", "miyu-wallpaper", "miyu-view"] {
        assert_eq!(
            manifest["dependencies"][dep]["optional"].as_bool(),
            Some(true),
            "{dep} 是可选的"
        );
    }
    let default = manifest["features"].get("default");
    assert!(default.is_none(), "没有默认打开的 feature：{default:?}");
}

/// 门面里的 `open`、`CoreCommand` 是挪来的那一份：网页、接入QQ 都照它连核心。
#[test]
fn the_face_lists_what_the_heads_use() {
    let kinds = [
        std::any::type_name::<crate::open::Core>(),
        std::any::type_name::<crate::CoreCommand>(),
    ];
    assert!(kinds[0].starts_with("miyu_client::open::"), "{kinds:?}");
}

/// 界面块的查验和挑语言是核心收 `widget.set` 时用的那一份（施工 X-28）：写对的收，写错的说错在哪，字照语言挑。
#[test]
fn widget_content_is_checked_and_localized_as_the_core_does() {
    use crate::widgets::{WidgetPlace, check, localize};
    let content = serde_json::json!([{"type": "text", "text": {"zh": "你好", "en": "Hi"}}]);
    assert_eq!(check(WidgetPlace::Sidebar, &[], &content), Ok(()));
    let bad = serde_json::json!([{"type": "nope"}]);
    assert!(check(WidgetPlace::Sidebar, &[], &bad).is_err());
    let picked = localize(&content, "en");
    assert_eq!(picked[0]["text"], "Hi", "{picked}");
}

/// 核心的拒绝照 `data.reason` 认、原话照留，登录被挡的带还要等多久（施工 W-8 再补）；没有原因的写 `refused`。
#[test]
fn a_refusal_keeps_its_reason_message_and_wait() {
    let throttled = serde_json::json!({
        "code": -32010,
        "message": "登录太频繁",
        "data": {"reason": "login_throttled", "retry_after_ms": 1500},
    });
    let refused = crate::open::Refused::of(&throttled);
    assert_eq!(
        (
            refused.reason.as_str(),
            refused.message.as_str(),
            refused.retry_after_ms
        ),
        ("login_throttled", "登录太频繁", Some(1500))
    );
    assert_eq!(refused.to_string(), "登录太频繁");
    let bare = crate::open::Refused::of(&serde_json::json!({}));
    assert_eq!(
        (bare.reason.as_str(), bare.retry_after_ms),
        ("refused", None)
    );
}

/// 视图流读回来（施工 V-D）：核心写出去的推送照原样读回来，写回去一字不差；核心以后加的格（这里的 `future`）不管，可以没有的格
/// 缺着照没有。各种条目、旁白、会话状态读回来一样，由 `miyu-view` 的测试照真会话的每一条钉（`tests/support/mod.rs` 的
/// `round_trip`）。
#[cfg(feature = "view")]
#[test]
fn the_view_stream_reads_back_and_ignores_fields_it_does_not_know() {
    use crate::view::{Body, Change, Notice, State, Status};
    let pushed = r#"{"op":"update","entry":{"id":"b5.0","kind":"tool","group":"g5.0","call":"call_6_1","name":"write","title":{"name":"Write","object":"a"},"state":"running","args":"{\"file_path\":\"a\"}","approval":{"access":"write","decision":"once","by":{"kind":"person","account":"alice"}},"turn":3,"at":"2026-09-25T07:00:04.000Z"}}"#;
    let change: Change = serde_json::from_str(pushed).expect("读得懂");
    assert_eq!(serde_json::to_string(&change).expect("写得出"), pushed);
    let later = r#"{"op":"add","entry":{"id":"e9","kind":"notice","what":"answered","questions":[],"answers":[],"by":{"kind":"kernel"},"future":{"x":1},"at":"2026-09-25T07:00:10.000Z"},"after":null}"#;
    let Change::Add { entry, .. } = serde_json::from_str(later).expect("不认识的格不管")
    else {
        panic!("是加一条");
    };
    assert!(matches!(entry.body, Body::Notice(Notice::Answered(_))));
    assert!(!entry.hidden, "没写的照没有");
    let status: Status = serde_json::from_str(r#"{"state":"idle","jobs":[],"future":true}"#)
        .expect("可以没有的格缺着");
    assert_eq!((status.state, status.waited_ms), (State::Idle, 0));
}
