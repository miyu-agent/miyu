//! 测试共用的：给人看的字照仓库里的资源读（和出厂一样），会话由内核的替身照剧本跑出来，投影喂两遍对照：只喂落了盘的
//! （翻页），落了盘的和瞬时的照推送的先后一起喂（视图流）。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

mod stage;
mod words;

pub use stage::{policy, policy_stage, stage, summarizes};
pub use words::texts;

use std::sync::Arc;

use miyu_kernel::testkit::{Stage, Streamed};
use miyu_view::{Body, Change, Entry, Notice, Projector};

/// 只喂落了盘的：翻页的样子。
pub fn page(stage: &Stage) -> Vec<Entry> {
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for event in stage.log() {
        projector.event(event);
    }
    projector.entries().to_vec()
}

/// 落了盘的和瞬时的照推送的先后一起喂：视图流的样子，连同每一条交出的变化。
pub fn live(stage: &Stage) -> (Vec<Entry>, Vec<Change>) {
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    let mut changes = Vec::new();
    for item in stage.stream() {
        changes.extend(match item {
            Streamed::Event(event) => projector.event(event),
            Streamed::Transient(transient) => projector.transient(transient),
        });
    }
    (projector.entries().to_vec(), changes)
}

/// 视图流照交出的变化一步步拼出来的样子：头收到推送以后手里的。
pub fn replayed(changes: &[Change]) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    let position = |entries: &[Entry], after: &Option<miyu_view::EntryId>| match after {
        None => 0,
        Some(id) => entries
            .iter()
            .position(|e| &e.id == id)
            .map_or(entries.len(), |i| i + 1),
    };
    for change in changes {
        match change {
            Change::Add { entry, after } => {
                let at = position(&entries, after);
                entries.insert(at, entry.clone());
            }
            Change::Update { entry, after } => {
                let old = entries.iter().position(|e| e.id == entry.id);
                match after {
                    Some(after) => {
                        if let Some(i) = old {
                            entries.remove(i);
                        }
                        let at = position(&entries, after);
                        entries.insert(at, entry.clone());
                    }
                    None => {
                        if let Some(i) = old {
                            entries[i] = entry.clone();
                        }
                    }
                }
            }
            Change::Append { id, text } => {
                if let Some(entry) = entries.iter_mut().find(|e| &e.id == id) {
                    match &mut entry.body {
                        Body::Reply(reply) => reply.text.push_str(text),
                        Body::Thought(thought) => thought.text.push_str(text),
                        Body::Tool(tool) => tool.args.push_str(text),
                        _ => {}
                    }
                }
            }
            Change::Hidden { ids, hidden } => {
                for entry in entries.iter_mut().filter(|e| ids.contains(&e.id)) {
                    entry.hidden = *hidden;
                }
            }
            Change::Remove { id } => entries.retain(|e| &e.id != id),
        }
    }
    entries
}

/// 翻页和视图流最后一样：只在视图流里有的旁白（`x…`）、只有推送才有的压缩前后的用量不算。
pub fn same(stage: &Stage) -> Vec<Entry> {
    let paged = page(stage);
    let (streamed, changes) = live(stage);
    let comparable = |entries: &[Entry]| -> Vec<Entry> {
        entries
            .iter()
            .filter(|e| !e.id.as_str().starts_with('x'))
            .cloned()
            .map(|mut e| {
                if let Body::Notice(Notice::Compaction(c)) = &mut e.body {
                    c.before = None;
                    c.after = None;
                }
                e
            })
            .collect()
    };
    assert_eq!(
        comparable(&streamed),
        comparable(&paged),
        "视图流和翻页最后一样"
    );
    assert_eq!(
        replayed(&changes),
        streamed,
        "照推送拼出来的和投影手里的一样"
    );
    // 头照门面把推送、条目读回来（施工 V-D）：每一条写成 JSON 再读回来一样。
    for change in &changes {
        round_trip(change);
        on_the_wire(change);
    }
    for entry in &paged {
        round_trip(entry);
    }
    paged
}

/// 写成 JSON 再读回来，和原来的一样（施工 V-D：终端照门面读视图流）。
pub fn round_trip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let text = serde_json::to_string(value).expect("写得成 JSON");
    let back: T = serde_json::from_str(&text).unwrap_or_else(|e| panic!("读不回来：{e}\n{text}"));
    assert_eq!(&back, value, "读回来不一样：{text}");
}

/// 条目写成 JSON，比较字段用。
pub fn json(entry: &Entry) -> serde_json::Value {
    serde_json::to_value(entry).expect("条目写得成 JSON")
}

/// 种类是 `kind` 的条目，照先后。
pub fn of_kind<'a>(entries: &'a [Entry], kind: &str) -> Vec<&'a Entry> {
    entries.iter().filter(|e| json(e)["kind"] == kind).collect()
}

/// 照视图流在线上的写法（`view.<op>` 加参数，带 `session`，没有 `op`）写一条推送，再照 [`Change::read`] 读回来，和原来的一样
/// （施工 V-D 修）。
pub fn on_the_wire(change: &Change) {
    let mut params = serde_json::to_value(change).expect("写得成 JSON");
    let op = params
        .as_object_mut()
        .and_then(|fields| fields.remove("op"))
        .and_then(|op| op.as_str().map(str::to_string))
        .expect("有 op");
    params["session"] = serde_json::json!("01a10000-0000-7000-8000-000000000001");
    let back = Change::read(&format!("view.{op}"), params.clone())
        .unwrap_or_else(|e| panic!("读不回来：{e}\n{params}"));
    assert_eq!(&back, change, "照线上的写法读回来不一样：{params}");
}
