//! 用出来的、供应商的列表：读写一字不差，只记小的（施工 8-7）。

use miyu_kernel::time::Timestamp;

use super::{Learned, ListedModel, ProviderList, Renamed, Stamped};

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("时刻写法对")
}

#[test]
fn a_learned_window_is_kept_only_when_it_is_smaller() {
    let mut learned = Learned::default();
    let first = at("2026-10-01T08:12:30.000Z");
    assert!(learned.learn("dev", "v4.1", 65_536, first));
    assert!(!learned.learn("dev", "v4.1", 65_536, at("2026-10-02T00:00:00.000Z")));
    assert!(!learned.learn("dev", "v4.1", 131_072, at("2026-10-02T00:00:00.000Z")));
    assert_eq!(
        learned.window("dev", "v4.1"),
        Some(Stamped {
            value: 65_536,
            at: first
        })
    );
    let later = at("2026-10-03T00:00:00.000Z");
    assert!(learned.learn("dev", "v4.1", 32_000, later));
    assert_eq!(
        learned.window("dev", "v4.1").map(|stamped| stamped.value),
        Some(32_000)
    );
    assert_eq!(learned.window("dev", "other"), None);
    assert_eq!(
        learned.to_json(),
        r#"{"dev/v4.1":{"window":{"value":32000,"at":"2026-10-03T00:00:00.000Z"}}}"#
    );
    assert_eq!(Learned::parse(&learned.to_json()), Ok(learned));
    assert!(Learned::parse("[]").is_err());
}

#[test]
fn a_provider_list_reads_back_the_same() {
    let list = ProviderList {
        fetched: at("2026-10-01T03:00:00.000Z"),
        models: vec![
            ListedModel {
                id: "deepseek-flash".to_string(),
                window: Some(1_000_000),
            },
            ListedModel {
                id: "x".to_string(),
                window: None,
            },
        ],
    };
    let text = list.to_json();
    assert_eq!(
        text,
        r#"{"fetched":"2026-10-01T03:00:00.000Z","models":[{"id":"deepseek-flash","window":1000000},{"id":"x"}]}"#
    );
    assert_eq!(ProviderList::parse(&text), Ok(list.clone()));
    assert_eq!(list.find("x").map(|listed| listed.window), Some(None));
    assert!(list.find("y").is_none());
    assert!(ProviderList::parse("{}").is_err());
}

/// 供应商改名（施工 8-34）：学到的跟着换；改名记下来，连着改的拉平，改回去的删掉。
#[test]
fn renaming_a_provider_moves_what_was_learned_and_is_remembered() {
    let at = at("2026-10-22T00:00:00.000Z");
    let mut learned = Learned::default();
    learned.learn("old", "m1", 1000, at);
    learned.learn("oldish", "m2", 2000, at);
    learned.rename_provider("old", "new");
    assert_eq!(learned.window("new", "m1").map(|w| w.value), Some(1000));
    assert_eq!(learned.window("old", "m1"), None);
    assert_eq!(
        learned.window("oldish", "m2").map(|w| w.value),
        Some(2000),
        "前缀像的不动"
    );

    let mut renamed = Renamed::default();
    renamed.record("a", "b");
    renamed.record("b", "c");
    assert_eq!(
        (renamed.current("a"), renamed.current("b")),
        (Some("c"), Some("c"))
    );
    renamed.record("c", "a");
    assert_eq!(renamed.current("a"), None, "改回去的删掉");
    assert_eq!(
        (renamed.current("b"), renamed.current("c")),
        (Some("a"), Some("a"))
    );
    let again = Renamed::parse(&renamed.to_json()).unwrap();
    assert_eq!(again, renamed);
}
