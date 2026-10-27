//! `provide` 的几条（施工 O-2 上）：访问类别、给哪种会话照写法查，不认识的、空的拒；提供者表照包记，连接断了只拿掉它自己的。
//! 时限照写法查（施工 O-2 下）；提供者的工具带上是谁要的，超时、叫它停、掐掉发 `tool.cancel`，回应先到的不发（`remote_tests.rs`）。

use serde_json::json;
use tokio::sync::mpsc;

use super::*;

fn tool(access: &str, venues: Value) -> ToolParams {
    serde_json::from_value(json!({
        "name": "send_group", "description": "Send to a group.",
        "input_schema": {"type": "object"}, "access": access, "venues": venues
    }))
    .expect("合写法")
}

fn refused_for(params: ToolParams) -> Value {
    let Err(refusal) = checked(params) else {
        panic!("应该拒");
    };
    assert_eq!(refusal.reason, "bad_tool");
    Value::Object(refusal.data.expect("带 data"))
}

#[test]
fn access_and_venues_are_checked() {
    let Checked { spec, venues, .. } =
        checked(tool("outbound", json!(["local", "group"]))).expect("合写法");
    assert_eq!(spec.access, Access::Outbound);
    assert_eq!(
        venues,
        Venues {
            local: true,
            private: false,
            group: true
        }
    );
    // 在场所里做的事（施工 O-31 前）：认 `venue`。
    let Checked { spec, .. } = checked(tool("venue", json!(["group"]))).expect("合写法");
    assert_eq!(spec.access, Access::Venue);
    assert_eq!(
        refused_for(tool("telepathy", json!(["local"]))),
        json!({"tool": "send_group", "problem": "access"})
    );
    assert_eq!(
        refused_for(tool("read", json!([]))),
        json!({"tool": "send_group", "problem": "venues"})
    );
    assert_eq!(
        refused_for(tool("read", json!(["moon"]))),
        json!({"tool": "send_group", "problem": "venues"})
    );
}

/// 提供者表照包记，重新登记的换掉旧的：交出来的是后来那个连接（往它那头发）。
#[tokio::test]
async fn the_table_follows_the_package_and_a_new_registration_replaces_the_old() {
    let provided = Provided::default();
    assert!(provided.peer("onebot").is_none());
    let (first_out, mut first) = mpsc::channel(1);
    let (second_out, mut second) = mpsc::channel(1);
    provided.register(Instance::system("onebot"), Peer::new(first_out));
    provided.register(Instance::system("onebot"), Peer::new(second_out));
    let peer = provided.peer("onebot").expect("有");
    tokio::spawn(async move { peer.call("tool.call", json!({})).await });
    assert!(second.recv().await.is_some(), "发给后来的那个");
    assert!(first.try_recv().is_err(), "旧的收不到");
}

/// 提供者的工具（施工 O-2 上）：没有连接的、等着时连接断了的，都是暂时不可用，说法同执行器替目录里没有的工具写的。
#[tokio::test]
async fn without_a_live_connection_a_provided_tool_is_unavailable() {
    let provided = Arc::new(Provided::default());
    let remote = RemoteTool::new(
        checked(tool("read", json!(["local"]))).expect("合写法"),
        "onebot",
        Arc::clone(&provided),
        texts(),
        stores().0,
    );
    let unavailable = |done: miyu_tool::Done| {
        assert!(done.error);
        assert_eq!(
            done.blocks,
            [miyu_kernel::block::Block::Text(miyu_kernel::block::Text {
                text: "The tool \"send_group\" is not available right now.\n".to_string()
            })]
        );
        assert_eq!(
            done.human,
            Some(
                miyu_kernel::event::Said::new("core/tool-results/unavailable")
                    .with("name", "send_group")
            )
        );
    };
    let progress = || miyu_tool::Progress::new(|_| {});
    unavailable(remote.run(miyu_tool::testkit::call("{}"), progress()).await);
    // 有连接，可是发不出去（那一头关了）。
    let (out, lines) = mpsc::channel(1);
    drop(lines);
    provided.register(Instance::system("onebot"), Peer::new(out));
    unavailable(remote.run(miyu_tool::testkit::call("{}"), progress()).await);
}

/// 两句的模板，照资源里的写。
fn texts() -> Texts {
    Texts {
        unavailable: "The tool \"{name}\" is not available right now.\n".to_string(),
        timed_out: "The tool \"{name}\" did not answer within {seconds} seconds.\n".to_string(),
    }
}

/// 扩展传上来的 blob 放在一个临时数据根里，提供者的账号是 `admin`（施工 O-2 三补）。交回目录，测完删。
fn stores() -> (Stores, std::path::PathBuf) {
    let (root, dir) = crate::test_support::temp_root("provide");
    let provider = miyu_kernel::id::AccountId::parse("admin").expect("合写法");
    (
        Stores {
            root,
            provider,
            per_account: false,
        },
        dir,
    )
}

/// 时限（施工 O-2 下）：不写是一分钟；一秒到十分钟，出了这个范围的拒，`problem` 是 `timeout`。
#[test]
fn the_timeout_is_checked() {
    let timed = |timeout: Value| {
        let mut params = serde_json::to_value(tool("read", json!(["local"]))).unwrap();
        params["timeout_ms"] = timeout;
        serde_json::from_value::<ToolParams>(params).unwrap()
    };
    let unset = checked(tool("read", json!(["local"]))).expect("合写法");
    assert_eq!(unset.timeout, Duration::from_secs(60));
    let fast = checked(timed(json!(1000))).expect("合写法");
    assert_eq!(fast.timeout, Duration::from_secs(1));
    for bad in [json!(999), json!(600_001)] {
        assert_eq!(
            refused_for(timed(bad)),
            json!({"tool": "send_group", "problem": "timeout"})
        );
    }
}

/// 退役的「要先加载」（施工 L-1 登记、FL-1 拆掉：工具一律全量）：写了的扩展照旧登记得上，写什么都一样。
#[test]
fn a_retired_need_load_is_accepted_whatever_it_says() {
    let marked = |need: Value| {
        let mut params = serde_json::to_value(tool("read", json!(["local"]))).unwrap();
        params["need_load"] = need;
        serde_json::from_value::<ToolParams>(params).unwrap()
    };
    let plain = checked(tool("read", json!(["local"])))
        .expect("合写法")
        .spec;
    for need in [json!(true), json!(false), json!("yes"), json!(1)] {
        let spec = checked(marked(need.clone())).expect("合写法").spec;
        assert_eq!(spec, plain, "{need}");
    }
}

/// 脚本里能调（施工 M-2）：`true` 的开，不写、`false` 的关；不是布尔的拒，`problem` 是 `clicall`。
#[test]
fn clicall_is_a_boolean() {
    let marked = |on: Value| {
        let mut params = serde_json::to_value(tool("read", json!(["local"]))).unwrap();
        params["clicall"] = on;
        serde_json::from_value::<ToolParams>(params).unwrap()
    };
    assert!(checked(marked(json!(true))).expect("合写法").spec.clicall);
    assert!(!checked(marked(json!(false))).expect("合写法").spec.clicall);
    assert!(
        !checked(tool("read", json!(["local"])))
            .expect("合写法")
            .spec
            .clicall
    );
    for bad in [json!("yes"), json!(1)] {
        assert_eq!(
            refused_for(marked(bad)),
            json!({"tool": "send_group", "problem": "clicall"}),
        );
    }
}

/// 结构化结果的样子（施工 M-4）：一行字收下，不写的没有；不是字的、空白的拒，`problem` 是 `result`。
#[test]
fn result_is_a_line_of_text() {
    let marked = |shape: Value| {
        let mut params = serde_json::to_value(tool("read", json!(["local"]))).unwrap();
        params["result"] = shape;
        serde_json::from_value::<ToolParams>(params).unwrap()
    };
    assert_eq!(
        checked(marked(json!(r#"{"memes":[{"id","tags"}]}"#)))
            .expect("合写法")
            .spec
            .result
            .as_deref(),
        Some(r#"{"memes":[{"id","tags"}]}"#)
    );
    assert_eq!(
        checked(tool("read", json!(["local"])))
            .expect("合写法")
            .spec
            .result,
        None
    );
    for bad in [json!("  "), json!(1), json!({"memes": []})] {
        assert_eq!(
            refused_for(marked(bad)),
            json!({"tool": "send_group", "problem": "result"}),
        );
    }
}

mod remote_tests;

/// 给人看的名字（施工 9-8 六补）：照语言收下，派出去那一刻交给执行器；不写的是空的；语言、名字是空白的拒。
#[test]
fn the_name_people_see_is_kept_and_blanks_are_refused() {
    let named = |display_name: Value| -> ToolParams {
        serde_json::from_value(json!({
            "name": "send_group", "description": "Send to a group.",
            "input_schema": {"type": "object"}, "access": "outbound", "venues": ["group"],
            "display_name": display_name
        }))
        .expect("合写法")
    };
    let Checked { display_name, .. } =
        checked(named(json!({"zh": "发到群里", "en": "Send to group"}))).expect("合写法");
    assert_eq!(
        display_name,
        BTreeMap::from([
            ("en".to_string(), "Send to group".to_string()),
            ("zh".to_string(), "发到群里".to_string()),
        ])
    );
    let Checked { display_name, .. } = checked(tool("outbound", json!(["group"]))).expect("合写法");
    assert!(display_name.is_empty(), "不写的是空的");
    for blank in [json!({"zh": " "}), json!({"": "发到群里"})] {
        assert_eq!(
            refused_for(named(blank)),
            json!({"tool": "send_group", "problem": "display_name"})
        );
    }
}

/// 显示名后面跟哪一个参数（施工 S-6 上）：写了的要是这件工具参数格式 `properties` 里的一个名字，收下；不写的是空的；指着不存在
/// 的参数、参数格式里没有 `properties` 的拒，同写错的显示名一类。
#[test]
fn the_subject_names_one_of_the_parameters() {
    let with = |schema: Value, subject: &str| -> ToolParams {
        serde_json::from_value(json!({
            "name": "send_group", "description": "Send to a group.",
            "input_schema": schema, "access": "outbound", "venues": ["group"],
            "subject": subject
        }))
        .expect("合写法")
    };
    let schema = json!({"type": "object", "properties": {"text": {"type": "string"}}});
    let Checked { subject, .. } = checked(with(schema.clone(), "text")).expect("合写法");
    assert_eq!(subject.as_deref(), Some("text"));
    let Checked { subject, .. } = checked(tool("outbound", json!(["group"]))).expect("合写法");
    assert_eq!(subject, None, "不写的是空的");
    for (schema, subject) in [(schema, "nope"), (schema_without_properties(), "text")] {
        assert_eq!(
            refused_for(with(schema, subject)),
            json!({"tool": "send_group", "problem": "subject"})
        );
    }
}

fn schema_without_properties() -> Value {
    json!({"type": "object"})
}
