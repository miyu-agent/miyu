//! 额度的纯逻辑（施工 QU-1）：档案的写法、配置盖地址、读 opencode Go 的回应。

use miyu_config::Value as ConfigValue;
use serde_json::json;

use super::*;
use crate::profile::Profiles;

/// 2026-11-14 实测的回应。
const REPLY: &str = r#"{"usage":{"rolling":{"status":"ok","percent":4,"resetsAt":"2026-11-14T08:31:28.000Z"},"weekly":{"status":"ok","percent":12.5,"resetsAt":"2026-11-17T00:00:00.000Z"},"monthly":{"status":"ok","percent":30,"resetsAt":"2026-12-01T00:00:00.000Z"}}}"#;

fn go() -> Quota {
    Quota {
        url: "https://opencode.ai/inference/go/v1/usage".to_string(),
        format: QuotaFormat::OpencodeGo,
    }
}

#[test]
fn the_profile_takes_a_quota_with_its_format() {
    let profiles = Profiles::parse(&json!({"providers": {"opencode-go": {
        "quota": {"url": "https://opencode.ai/inference/go/v1/usage", "format": "opencode-go"}
    }}}))
    .expect("读得进来");
    assert_eq!(profiles.providers["opencode-go"].quota, Some(go()));
    for wrong in [
        json!({"url": "https://x.invalid"}),
        json!({"url": "https://x.invalid", "format": "nope"}),
        json!({"url": "https://x.invalid", "format": "opencode-go", "more": 1}),
    ] {
        let parsed = Profiles::parse(&json!({"providers": {"a": {"quota": wrong}}}));
        assert!(parsed.is_err(), "{wrong}");
    }
}

#[test]
fn the_config_moves_the_address_but_needs_the_profile() {
    let mut values = Values::default();
    assert_eq!(settled(Some(&go()), &values, "opencode-go"), Some(go()));
    values.set(
        "providers.opencode-go.quota.url",
        ConfigValue::Text("https://moved.invalid/usage".into()),
    );
    let moved = settled(Some(&go()), &values, "opencode-go").expect("档案有");
    assert_eq!(moved.url, "https://moved.invalid/usage");
    assert_eq!(moved.format, QuotaFormat::OpencodeGo);
    assert_eq!(
        settled(None, &values, "opencode-go"),
        None,
        "格式只有档案知道"
    );
}

#[test]
fn the_go_reply_reads_into_three_windows_in_order() {
    let windows = read(QuotaFormat::OpencodeGo, REPLY.as_bytes()).expect("读得出");
    let ids: Vec<_> = windows.iter().map(|window| window.id.as_str()).collect();
    assert_eq!(ids, ["rolling", "weekly", "monthly"]);
    assert_eq!(windows[0].percent, Number::from(4));
    assert_eq!(windows[1].percent.as_f64(), Some(12.5));
    assert_eq!(windows[0].status, "ok");
    assert_eq!(windows[0].resets_at.to_string(), "2026-11-14T08:31:28.000Z");
}

#[test]
fn a_missing_window_is_left_out_and_a_broken_one_spoils_the_reply() {
    let one =
        r#"{"usage":{"weekly":{"status":"ok","percent":1,"resetsAt":"2026-11-17T00:00:00.000Z"}}}"#;
    let windows = read(QuotaFormat::OpencodeGo, one.as_bytes()).expect("读得出");
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].id, WindowId::Weekly);
    for broken in [
        "not json",
        "{}",
        r#"{"usage":{}}"#,
        r#"{"usage":{"rolling":{"status":"ok","percent":"4","resetsAt":"2026-11-14T08:31:28.000Z"}}}"#,
        r#"{"usage":{"rolling":{"percent":4,"resetsAt":"2026-11-14T08:31:28.000Z"}}}"#,
        r#"{"usage":{"rolling":{"status":"ok","percent":4,"resetsAt":"tomorrow"}}}"#,
    ] {
        assert!(
            read(QuotaFormat::OpencodeGo, broken.as_bytes()).is_err(),
            "{broken}"
        );
    }
}

#[test]
fn reset_times_read_any_rfc3339_form_into_utc() {
    let at = |text: &str| {
        let reply = format!(
            r#"{{"usage":{{"rolling":{{"status":"ok","percent":1,"resetsAt":"{text}"}}}}}}"#
        );
        read(QuotaFormat::OpencodeGo, reply.as_bytes())
            .ok()
            .map(|windows| windows[0].resets_at.to_string())
    };
    let wanted = Some("2026-11-14T08:31:28.000Z".to_string());
    for same in [
        "2026-11-14T08:31:28.000Z",
        "2026-11-14T08:31:28Z",
        "2026-11-14t08:31:28z",
        "2026-11-14 08:31:28Z",
        "2026-11-14T08:31:28.000123Z",
        "2026-11-14T17:31:28+09:00",
        "2026-11-14T03:01:28-05:30",
        "2026-11-14T08:31:28+00:00",
    ] {
        assert_eq!(at(same), wanted, "{same}");
    }
    assert_eq!(
        at("2026-11-14T08:31:28.5Z"),
        Some("2026-11-14T08:31:28.500Z".to_string())
    );
    for broken in [
        "2026-11-14T08:31:28",
        "2026-11-14T08:31:28.Z",
        "2026-11-14T08:31:28+0900",
        "2026-11-14T08:31:28+24:00",
        "2026-11-14T08:31:28++9:00",
        "2026-11-14X08:31:28Z",
        "2026-02-30T08:31:28Z",
        "2026-11-14T08:31:60Z",
    ] {
        assert_eq!(at(broken), None, "{broken}");
    }
}
