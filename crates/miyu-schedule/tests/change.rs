//! `note_change`（施工 LF-11，`docs/blueprint/tools/note_change.md`）：假的记忆端口。给 `text` 的交 `note`、给 `drop` 的交
//! `drop_note`，参数原样（「没传」的几种写法当没传）；`text`、`drop` 都没给、都给了、超长、编号写错、没有端口的不交端口，各回各的
//! 一句；端口拒的几种各回各的一句。规矩（只碰 `change`、上限）在真端口后面，在 `miyu-session` 和协议端点里测。

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::json;

use miyu_kernel::id::Seq;
use miyu_recall::MemoryId;
use miyu_tool::{
    Done, MemoryPort, Note, NoteRefused, Pending, Progress, Refused, Remember, Searched,
};

use crate::support::{ids, said, text};

fn m(n: u64) -> MemoryId {
    MemoryId::new(Seq::new(n).expect("从 1 起"))
}

/// 假的记忆端口：记下交来的，照预先定好的交回。
#[derive(Default)]
struct Fake {
    noted: Mutex<Vec<Note>>,
    dropped: Mutex<Vec<(MemoryId, String)>>,
    refuse: Option<NoteRefused>,
}

impl Fake {
    fn refusing(refused: NoteRefused) -> Arc<Fake> {
        Arc::new(Fake {
            refuse: Some(refused),
            ..Fake::default()
        })
    }

    fn noted(&self) -> Vec<Note> {
        self.noted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn dropped(&self) -> Vec<(MemoryId, String)> {
        self.dropped
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn answer<T>(&self, ok: T) -> Result<T, NoteRefused> {
        self.refuse.clone().map_or(Ok(ok), Err)
    }
}

impl MemoryPort for Fake {
    fn save<'a>(&'a self, _remember: Remember) -> Pending<'a, Result<MemoryId, Refused>> {
        panic!("note_change 不该调 save")
    }

    fn retire<'a>(&'a self, _id: MemoryId, _why: String) -> Pending<'a, Result<(), Refused>> {
        panic!("note_change 不该调 retire")
    }

    fn note<'a>(&'a self, note: Note) -> Pending<'a, Result<MemoryId, NoteRefused>> {
        self.noted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(note);
        let answer = self.answer(m(7));
        Box::pin(async move { answer })
    }

    fn drop_note<'a>(&'a self, id: MemoryId, why: String) -> Pending<'a, Result<(), NoteRefused>> {
        self.dropped
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((id, why));
        let answer = self.answer(());
        Box::pin(async move { answer })
    }

    fn search<'a>(
        &'a self,
        _query: String,
        _forgotten: bool,
    ) -> Pending<'a, Result<Searched, String>> {
        panic!("note_change 不该搜")
    }
}

/// 用端口 `port`（没有的是这个会话没开记忆）调一次 `note_change`。
async fn run(port: Option<&Arc<Fake>>, args: &str) -> Done {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = miyu_schedule::life_tools(&resources).expect("读得出");
    let tool = tools
        .iter()
        .find(|tool| tool.spec().name == "note_change")
        .expect("人生这个包里有 note_change");
    let mut call = miyu_tool::testkit::call(args);
    call.memory = port.map(|port| Arc::clone(port) as Arc<dyn MemoryPort>);
    call.ids = Some(ids());
    tool.run(call, Progress::new(|_| {})).await
}

/// 验收第 1 条：给 `text` 的交 `note`，回 `Noted as m…`；带 `replaces` 的回替换了哪一条；给 `drop` 的交 `drop_note`，`why`
/// 原样，没写的是空的。
#[tokio::test]
async fn text_notes_replaces_updates_and_drop_drops() {
    let port = Arc::new(Fake::default());
    let words = "以前十一点就睡，这阵子常看书到十二点多";
    let done = run(Some(&port), &json!({"text": words}).to_string()).await;
    assert_eq!(
        (done.error, text(&done)),
        (false, "Noted as m7.".to_string())
    );
    assert_eq!(
        said(&done),
        (
            "software/life/change/saved".to_string(),
            vec![("id".to_string(), "m7".to_string())]
        )
    );
    let done = run(
        Some(&port),
        &json!({"text": "现在常看书到一点", "replaces": "m3", "why": "书越看越长"}).to_string(),
    )
    .await;
    assert_eq!(
        (done.error, text(&done)),
        (false, "Noted as m7, in place of m3.".to_string())
    );
    assert_eq!(said(&done).0, "software/life/change/replaced");
    assert_eq!(
        port.noted(),
        [
            Note {
                text: words.to_string(),
                replaces: None,
                why: None
            },
            Note {
                text: "现在常看书到一点".to_string(),
                replaces: Some(m(3)),
                why: Some("书越看越长".to_string())
            },
        ]
    );
    let done = run(Some(&port), r#"{"drop":"m3","why":"又早睡了"}"#).await;
    assert_eq!(
        (done.error, text(&done)),
        (false, "Dropped m3.".to_string())
    );
    assert_eq!(said(&done).0, "software/life/change/dropped");
    let done = run(Some(&port), r#"{"drop":"m4","text":"","replaces":null}"#).await;
    assert_eq!(text(&done), "Dropped m4.", "没传的几种写法当没传");
    assert_eq!(
        port.dropped(),
        [(m(3), "又早睡了".to_string()), (m(4), String::new())]
    );
}

/// 验收第 2 条：`text`、`drop` 都没给、都给了，`drop` 带 `replaces`，超长，编号写错，参数读不懂：不交端口，各回各的一句。
#[tokio::test]
async fn bad_asks_never_reach_the_port() {
    let port = Arc::new(Fake::default());
    let one_of = "Give text to note a change, or drop alone to drop one.";
    let long: String = "长".repeat(121);
    let cases = [
        ("{}".to_string(), one_of.to_string(), "change/one-of"),
        (
            r#"{"text":"null","drop":"undefined","why":"x"}"#.to_string(),
            one_of.to_string(),
            "change/one-of",
        ),
        (
            r#"{"text":"变了","drop":"m3"}"#.to_string(),
            one_of.to_string(),
            "change/one-of",
        ),
        (
            r#"{"drop":"m3","replaces":"m2"}"#.to_string(),
            one_of.to_string(),
            "change/one-of",
        ),
        (
            r#"{"replaces":"m2"}"#.to_string(),
            one_of.to_string(),
            "change/one-of",
        ),
        (
            json!({"text": long}).to_string(),
            "That is 121 characters; a note is at most 120.".to_string(),
            "change/too-long",
        ),
        (
            r#"{"text":"变了","replaces":"three"}"#.to_string(),
            "There is no memory three.".to_string(),
            "memory/common/no-such",
        ),
        (
            r#"{"drop":"m0"}"#.to_string(),
            "There is no memory m0.".to_string(),
            "memory/common/no-such",
        ),
        (
            r#"{"text":3}"#.to_string(),
            "Bad arguments: invalid type: integer `3`, expected a string at line 1 column 9."
                .to_string(),
            "memory/common/bad-args",
        ),
    ];
    for (args, line, key) in cases {
        let done = run(Some(&port), &args).await;
        assert!(done.error, "{args}");
        assert_eq!(text(&done), line, "{args}");
        let key = match key.strip_prefix("memory/") {
            Some(rest) => format!("software/memory/{rest}"),
            None => format!("software/life/{key}"),
        };
        assert_eq!(said(&done).0, key, "{args}");
    }
    assert!(
        port.noted().is_empty() && port.dropped().is_empty(),
        "什么都没交"
    );
    let done = run(Some(&port), &json!({"text": "长".repeat(120)}).to_string()).await;
    assert!(!done.error, "刚好 120 个字的记得下");
}

/// 验收第 2 条：端口拒的几种各回各的一句。
#[tokio::test]
async fn refusals_from_the_port_are_said_in_their_own_words() {
    let cases = [
        (
            NoteRefused::Memory(Refused::NoSuch(m(9))),
            "There is no memory m9.",
            "software/memory/common/no-such",
        ),
        (
            NoteRefused::Memory(Refused::NotCurrent(m(9))),
            "m9 was already replaced or retired.",
            "software/memory/common/not-current",
        ),
        (
            NoteRefused::NotANote(m(9)),
            "m9 is not one of your notes of change.",
            "software/life/change/not-a-note",
        ),
        (
            NoteRefused::Full(10),
            "You already keep 10 notes. Update one with replaces, or drop one first.",
            "software/life/change/full",
        ),
        (
            NoteRefused::Memory(Refused::Failed("disk full".into())),
            "Memory is not available right now: disk full.",
            "software/memory/common/failed",
        ),
    ];
    for (refused, line, key) in cases {
        let port = Fake::refusing(refused.clone());
        for args in [r#"{"text":"变了","replaces":"m9"}"#, r#"{"drop":"m9"}"#] {
            let done = run(Some(&port), args).await;
            assert_eq!(
                (done.error, text(&done)),
                (true, line.to_string()),
                "{refused:?} {args}"
            );
            assert_eq!(said(&done).0, key, "{refused:?}");
        }
    }
}

/// 验收第 2 条：会话没开记忆（没有端口）的回那一句。
#[tokio::test]
async fn without_a_memory_port_memory_is_off() {
    for args in [r#"{"text":"变了"}"#, r#"{"drop":"m3"}"#] {
        let done = run(None, args).await;
        assert_eq!(
            (done.error, text(&done)),
            (true, "Memory is off in this session.".to_string())
        );
        assert_eq!(said(&done).0, "software/memory/common/off");
    }
}

/// 访问类别是读（同 `remember`、`end_life`）；四格参数都可以不写。
#[test]
fn note_change_reads_and_has_four_optional_parameters() {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = miyu_schedule::life_tools(&resources).expect("读得出");
    let names: Vec<&str> = tools.iter().map(|tool| tool.spec().name.as_str()).collect();
    assert_eq!(names, ["end_life", "note_change"]);
    let spec = tools[1].spec();
    assert_eq!(spec.access, miyu_kernel::tool::Access::Read);
    let parameters: serde_json::Value =
        serde_json::from_str(spec.parameters.get()).expect("参数是 JSON");
    let mut properties: Vec<&String> = parameters["properties"]
        .as_object()
        .expect("有几格")
        .keys()
        .collect();
    properties.sort();
    assert_eq!(properties, ["drop", "replaces", "text", "why"]);
    assert!(parameters.get("required").is_none(), "都可以不写");
}
