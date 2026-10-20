use super::*;

fn found(text: &str) -> Vec<String> {
    let mut found = BTreeSet::new();
    scan(text, &mut found, true);
    found.into_iter().collect()
}

#[test]
fn dispatch_positions_count() {
    let text = r#"
match method {
    "session.send" => send(),
    "blob.open" | "blob.write" => upload(),
    other if other == "x" => {}
}
match (method, peer) {
    ("account.setup", Some(_)) => {}
}
peer.call("tool.call", params);
json!({"jsonrpc": "2.0", "method": "content.changed"});
let method = if on { "widget.watch" } else { "widget.unwatch" };
"#;
    assert_eq!(
        found(text),
        [
            "account.setup",
            "blob.open",
            "blob.write",
            "content.changed",
            "session.send",
            "tool.call",
            "widget.unwatch",
            "widget.watch"
        ]
    );
}

#[test]
fn other_strings_do_not_count() {
    let text = r#"
let file = "config.schema.json";
tracing::info!(target: "miyu.endpoint", "done");
let key = format!("{}.{}", a, b);
let skip = "Session.Send" ;
"#;
    assert!(found(text).is_empty(), "{:?}", found(text));
}

#[test]
fn registered_lists_count_only_where_something_is_registered() {
    let list = "registry.register(&[\n    \"link.preview\",\n]);\n";
    assert_eq!(found(list), ["link.preview"]);
    assert!(found("let x = [\n    \"link.preview\",\n];\n").is_empty());
}

#[test]
fn inline_test_modules_are_skipped() {
    let text = "#[cfg(test)]\nmod tests {\n    match m { \"probe.echo\" => {} }\n}\n";
    assert!(found(text).is_empty());
}

#[test]
fn only_the_first_cell_of_a_table_row_is_an_entry() {
    assert_eq!(quoted("| `blob.put` "), ["blob.put"]);
    assert!(quoted("plain `text` and `a.b`").contains(&"a.b".to_string()));
    assert!(!is_method("Blob.put"));
    assert!(!is_method("blob."));
}
