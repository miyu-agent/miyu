//! 剧本参数里的占位怎么换（施工 LW-30）。

use super::*;

#[test]
fn placeholders_become_the_remembered_values() {
    let state = State {
        session: json!("s-last"),
        seq: json!(17),
        saved: HashMap::from([("g1".to_string(), json!("s-group"))]),
        next: 0,
    };
    let params = json!({"session": "{s:g1}", "to": ["{seq}"], "other": "{session}", "text": "{session} stays text",
        "missing": "{s:nope}", "n": 3});
    assert_eq!(
        state.fill(&params),
        json!({"session": "s-group", "to": [17], "other": "s-last", "text": "{session} stays text",
            "missing": null, "n": 3})
    );
}

#[test]
fn answers_have_the_shapes_the_core_reads() {
    let link = Link {
        out: Mutex::new(std::io::stdout()),
        waiting: Mutex::new(HashMap::new()),
    };
    let guard = json!({"id": "c1", "method": "tool.guard", "params": {"tool": "shell"}});
    assert_eq!(answer(&guard, &link), json!({"verdict": "allow"}));
    let append = json!({"id": "c2", "method": "user.append", "params": {"venue": "group"}});
    assert_eq!(answer(&append, &link)["content"][0]["kind"], "perf-hint");
    let search = json!({"id": "c3", "method": "search.query", "params": {"query": "q"}});
    assert_eq!(
        answer(&search, &link)["hits"].as_array().map(Vec::len),
        Some(5)
    );
}
