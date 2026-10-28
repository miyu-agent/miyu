//! `usage.query` 的参数（施工 8-15）：时区的写法、分组的名字、时刻、会话编号，不对的 `bad_params`；不写时区的照核心所在机器
//! 此刻的；只写 `tree` 不写会话的不算。

use serde_json::json;

use super::*;

fn params(value: Value) -> QueryParams {
    serde_json::from_value(value).unwrap()
}

/// 核心所在机器此刻的时区：测试里定成 +08:00。
fn here() -> UtcOffset {
    UtcOffset::from_minutes(480).unwrap()
}

#[test]
fn offsets_are_written_with_a_sign_and_minutes() {
    for (text, minutes) in [
        ("+09:00", 540),
        ("-05:30", -330),
        ("+00:00", 0),
        ("+14:00", 840),
    ] {
        assert_eq!(offset(text), UtcOffset::from_minutes(minutes), "{text}");
    }
    for text in [
        "09:00",
        "+9:00",
        "+09:60",
        "+14:01",
        "+15:00",
        "UTC+09:00",
        "+0900",
        "+09:0x",
        "",
    ] {
        assert_eq!(offset(text), None, "{text}");
    }
}

#[test]
fn the_parameters_are_read_and_checked() {
    let query = read(
        params(json!({
            "from": "2026-10-01T00:00:00.000Z",
            "until": null,
            "group": ["model", "day", "purpose"],
            "session": "01900000-0000-7000-8000-000000000001",
            "tree": true,
            "offset": "+05:30",
            "whatever": 1,
        })),
        here,
    )
    .unwrap();
    assert_eq!(query.group, [Group::Model, Group::Day, Group::Purpose]);
    assert_eq!(query.offset, UtcOffset::from_minutes(330).unwrap());
    assert_eq!(query.until, None);
    assert!(query.from.is_some());
    assert!(matches!(&query.session, Some((_, true))));
    let plain = read(params(json!({"tree": true})), here).unwrap();
    assert_eq!(
        (plain.session, plain.offset, plain.group),
        (None, here(), Vec::new())
    );
    for bad in [
        json!({"group": ["provider"]}),
        json!({"group": ["Model"]}),
        json!({"from": "yesterday"}),
        json!({"session": "abc"}),
        json!({"offset": "Asia/Tokyo"}),
    ] {
        let read = read(params(bad.clone()), here);
        assert!(matches!(read, Err(Refusal::BAD_PARAMS)), "{bad}");
    }
}

/// 金额哪个币种排最前照系统配置（SU-3 上起只有这一份）：不写是出厂的 USD，写了 CNY 的谁的会话都照 CNY。
#[test]
fn amounts_lead_with_the_configured_currency() {
    let (root, dir) = crate::test_support::temp_root("usage-currency");
    let alice = AccountId::parse("alice").expect("合写法");
    let bob = AccountId::parse("bob").expect("合写法");
    let settings = root.path().join("config.toml");
    std::fs::create_dir_all(settings.parent().expect("有上一层")).expect("建得出");
    std::fs::write(&settings, "[usage]\ncurrency = \"CNY\"\n").expect("写得进");
    let config = crate::config::Config::load(
        &root,
        &alice,
        None,
        miyu_models::settings::UsageSettings::ITEMS.to_vec(),
        crate::config::Environment::of(&[]),
    );
    let core = Core::new(
        root,
        miyu_store::resources::ResourceRoot::at(crate::test_support::resources()),
        Arc::new(miyu_session::testkit::Script::new([])),
        miyu_tool::Catalog::default(),
        None,
        alice.clone(),
        "token".to_string(),
    )
    .with_config(config);
    let mut tally = miyu_session::Tally::default();
    tally.amounts.insert("CNY".to_string(), 1.0);
    tally.amounts.insert("USD".to_string(), 2.0);
    let first = |shown: Value| shown["amounts"][0]["currency"].clone();
    assert_eq!(currency(&core, &alice), "CNY");
    assert_eq!(currency(&core, &bob), "CNY");
    assert_eq!(first(tallied(&core, &alice, &tally)), json!("CNY"));
    assert_eq!(first(tallied(&core, &bob, &tally)), json!("CNY"));
    assert_eq!(first(total_row(&core, &bob, own(&tally))), json!("CNY"));
    // 换算也照配置的币种（施工 UC-1）：没装取汇率的，只换得了它自己那个币种的那一笔，别的在 `rest`。
    tally.daily.insert((20_000, "CNY".to_string()), 1.0);
    tally.daily.insert((20_000, "USD".to_string()), 2.0);
    assert_eq!(
        tallied(&core, &bob, &tally)["converted"],
        json!({"currency": "CNY", "amount": 1, "rest": [{"currency": "USD", "amount": 2}]})
    );
    drop(dir);
}

/// 两份加起来（会话树里子会话的、会话自己的，施工 UC-1）：按天分的金额跟着加，换算照加起来的算。
#[test]
fn adding_two_totals_keeps_the_amounts_by_day() {
    let mut one = empty();
    one.amounts = vec![("USD".to_string(), 1.0)];
    one.daily = vec![(20_000, "USD".to_string(), 1.0)];
    let mut other = empty();
    other.amounts = vec![("CNY".to_string(), 3.0)];
    other.daily = vec![(20_001, "CNY".to_string(), 3.0)];
    let sum = plus(one, &other);
    assert_eq!(
        sum.daily,
        vec![
            (20_000, "USD".to_string(), 1.0),
            (20_001, "CNY".to_string(), 3.0)
        ]
    );
}
