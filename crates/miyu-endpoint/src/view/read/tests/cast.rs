//! 翻页时的人格、预设、权限级别（施工 FX-16）：照对照测试的造法（`same::made`，开头的 `session.created` 选了人格、预设，中途有人
//! 换、内核换上快照）造会话日志，从最新一页一页翻到头（一页一轮、两轮），每一页里回复的 `persona`、换人格的旁白、消息的
//! `level` 和整份喂投影的那几条一样。改之前切点前的 `session.created`、`session.policy_changed` 不交给投影学，后面几页的回复
//! 全是 `null`。

use serde_json::Value;

use miyu_store::blob::Blobs;
use miyu_store::log::read_events;
use miyu_view::Entry;

use super::same::{made, texts};
use crate::test_support::temp_root;
use crate::view::{CAP, paged, project, read::read, size};

/// 条目里的人格、预设、权限级别：回复的 `persona`、换人格的旁白、消息的 `level`，照编号。
fn cast(entries: &[Entry]) -> Vec<(String, Value)> {
    entries
        .iter()
        .map(|entry| serde_json::to_value(entry).expect("条目写得成 JSON"))
        .filter_map(|e| {
            let id = e["id"].as_str().unwrap_or_default().to_string();
            match e["kind"].as_str() {
                Some("reply") => Some((id, e["persona"].clone())),
                Some("user") => Some((id, e["level"].clone())),
                // 撤销藏起的照后面的日志定：更早的一页读不到后来的撤销，`hidden` 不比。
                Some("notice") if e["what"] == "persona" => {
                    let mut e = e;
                    if let Some(fields) = e.as_object_mut() {
                        fields.remove("hidden");
                    }
                    Some((id, e))
                }
                _ => None,
            }
        })
        .collect()
}

#[test]
fn every_page_names_the_persona_the_whole_log_does() {
    let (root, place) = temp_root("fx16-cast");
    let blobs = Blobs::new(root.blobs());
    let texts = texts();
    let mut later = 0;
    for seed in 0..40 {
        let dir = place.join(format!("s{seed}"));
        made(seed, &dir);
        let events = read_events(&dir).expect("造的日志读得成");
        let mut whole = project::fresh(texts.clone(), blobs.clone());
        for event in &events {
            whole.event(event);
        }
        let whole = cast(whole.entries());
        for turns in [1, 2] {
            let mut before = None;
            loop {
                let slice = read(&dir, before, (turns, CAP), true, &size)
                    .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
                let (reply, projector) = paged(&slice, Some((texts.clone(), blobs.clone())));
                let page = cast(projector.expect("交了条目").entries());
                // 照这一页的先后比：带进来的触发消息、排着的话在页里的位置可以和整份的不一样，比的是每一条的格。
                let expected: Vec<_> = page
                    .iter()
                    .filter_map(|(id, _)| whole.iter().find(|(seen, _)| seen == id).cloned())
                    .collect();
                assert_eq!(
                    page, expected,
                    "seed {seed} turns {turns} before {before:?}"
                );
                match (reply["first"].as_u64(), reply["more"] == Value::Bool(true)) {
                    (Some(first), true) => {
                        before = Some(first);
                        later += usize::from(!page.is_empty());
                    }
                    _ => break,
                }
            }
        }
        miyu_store::testkit::remove_tree(&dir);
    }
    assert!(later > 0, "有切在开头以后、带回复或消息的页");
}
