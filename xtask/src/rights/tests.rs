use super::listed;

#[test]
fn only_method_shaped_literals_count() {
    let found = listed(
        "match method { \"session.send\" | \"kb.add\" => x, \"hello\" => y, \"a.\" => z }\n#[cfg(test)]\n\"no.counted\"",
    );
    let found: Vec<&str> = found.iter().map(String::as_str).collect();
    assert_eq!(found, ["kb.add", "session.send"]);
}
