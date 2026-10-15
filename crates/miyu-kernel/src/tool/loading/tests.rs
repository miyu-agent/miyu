//! 答 `load_tools`、附上完整格式（施工 L-1）：要的几件照先后、重复的一次；不用加载、不存在的最后一句说；参数不对的不成；
//! 完整格式照原样，引号不转义；不是要先加载的没有附的。

use super::*;

fn loading() -> Loading {
    let contracts = BTreeMap::from([
        (
            "kb_search".to_string(),
            Contract {
                description: "Search \"notes\".".to_string(),
                parameters: serde_json::from_str(
                    r#"{"type":"object","properties":{"q":{"type":"string"}}}"#,
                )
                .unwrap(),
            },
        ),
        (
            "kb_read".to_string(),
            Contract {
                description: "Read a note.".to_string(),
                parameters: serde_json::from_str(r#"{"type":"object"}"#).unwrap(),
            },
        ),
    ]);
    Loading::new(
        "load_tools".to_string(),
        contracts,
        LoadingSources {
            unknown: "Nothing to load for {names}.\n",
            bad_args: "List tool names in tools.\n",
            contract: "Full parameters of {name}: ",
        },
    )
    .unwrap()
}

#[test]
fn the_asked_contracts_come_back_in_order_once_each() {
    let (ok, worded) = loading().answer(r#"{"tools":["kb_search","kb_read","kb_search"]}"#);
    assert!(ok);
    assert_eq!(
        worded.text,
        concat!(
            r#"{"name":"kb_search","description":"Search \"notes\".","parameters":{"type":"object","properties":{"q":{"type":"string"}}}}"#,
            "\n",
            r#"{"name":"kb_read","description":"Read a note.","parameters":{"type":"object"}}"#,
            "\n"
        )
    );
    assert_eq!(
        worded.said,
        Some(Said::new("core/loading/loaded").with("tools", "kb_search, kb_read"))
    );
}

#[test]
fn names_that_need_no_loading_are_said_at_the_end() {
    let (ok, worded) = loading().answer(r#"{"tools":["read","kb_read","read"]}"#);
    assert!(ok);
    assert!(
        worded.text.ends_with("\nNothing to load for read.\n"),
        "{}",
        worded.text
    );
    let (ok, worded) = loading().answer(r#"{"tools":[]}"#);
    assert!(ok);
    assert_eq!(worded.text, "");
}

#[test]
fn without_a_list_of_names_it_fails() {
    for args in [r#"{}"#, r#"{"tools":"kb_read"}"#, r#"{"tools":[1]}"#] {
        let (ok, worded) = loading().answer(args);
        assert!(!ok, "{args}");
        assert_eq!(worded.text, "List tool names in tools.\n");
        assert_eq!(worded.said, Some(Said::new("core/loading/bad-args")));
    }
}

#[test]
fn only_tools_that_need_loading_get_their_contract_appended() {
    assert_eq!(
        loading().contract_of("kb_read").as_deref(),
        Some("Full parameters of kb_read: {\"type\":\"object\"}\n")
    );
    assert_eq!(loading().contract_of("read"), None);
}
