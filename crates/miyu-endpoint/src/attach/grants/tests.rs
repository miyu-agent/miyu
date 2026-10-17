use serde_json::json;

use super::*;

fn hash(text: &str) -> ContentHash {
    ContentHash::of(text.as_bytes())
}

#[test]
fn a_package_holds_only_what_it_was_granted() {
    let grants = Grants::default();
    grants.grant("a", &hash("one"));
    assert!(grants.holds("a", &hash("one")));
    assert!(!grants.holds("a", &hash("two")));
    assert!(!grants.holds("b", &hash("one")));
}

#[test]
fn an_answer_passes_on_only_what_the_answering_package_holds() {
    let grants = Grants::default();
    grants.grant("answers", &hash("own"));
    let answer = json!({
        "picture": hash("own").as_str(),
        "nested": [{"blob": hash("seen elsewhere").as_str()}, "not a hash", 3],
    });
    grants.pass("answers", "asks", &answer);
    assert!(grants.holds("asks", &hash("own")));
    assert!(!grants.holds("asks", &hash("seen elsewhere")));
    grants.pass("holds nothing", "asks", &json!([hash("other").as_str()]));
    assert!(!grants.holds("asks", &hash("other")));
}

#[test]
fn a_removed_package_starts_over() {
    let grants = Grants::default();
    grants.grant("a", &hash("one"));
    grants.forget("a");
    assert!(!grants.holds("a", &hash("one")));
}
