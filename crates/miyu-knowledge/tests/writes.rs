//! 知识库写的三件工具（施工 K-3，`docs/blueprint/knowledge.md`「她的工具」）：说的话、报的效果（改前改后的内容、回收站里的
//! 位置），`append` 不写是新建，写的拒绝如实说、没写成的不报效果。端口是假的：真的在 `miyu-session` 里测。

use std::path::PathBuf;
use std::sync::{Arc, PoisonError};

use miyu_tool::{Effect, KbRefused, KbSave};

use crate::support::*;

#[tokio::test]
async fn saving_says_what_it_did_and_reports_the_change() {
    for (written, args, line, said_key) in [
        (
            written(None, "て形\n", false),
            serde_json::json!({"kb": "notes", "path": "a.md", "text": "て形"}),
            "Saved a.md in notes.",
            "kb_save/saved",
        ),
        (
            written(Some("て形\n"), "て形\n食べて\n", false),
            serde_json::json!({"kb": "notes", "path": "a.md", "text": "食べて", "append": true}),
            "Appended to a.md in notes.",
            "kb_save/appended",
        ),
        (
            written(None, "红烧肉\n", true),
            serde_json::json!({"kb": "recipes", "path": "a.md", "text": "红烧肉", "about": "家常菜"}),
            "Created knowledge base recipes and saved a.md.",
            "kb_save/created",
        ),
    ] {
        let port = Arc::new(Fake {
            written: Some(written.clone()),
            ..Fake::default()
        });
        let done = call(Some(Arc::clone(&port)), "kb_save", args).await;
        assert!(!done.error);
        assert_eq!(text(&done), line);
        assert_eq!(said(&done), format!("software/knowledge/{said_key}"));
        assert_eq!(
            done.effects,
            [Effect::Changed {
                path: written.path.clone(),
                before: written.before.clone(),
                after: written.after.clone(),
            }],
            "撤销照它改回去"
        );
    }
    let port = Arc::new(Fake {
        written: Some(written(None, "x\n", true)),
        ..Fake::default()
    });
    call(
        Some(Arc::clone(&port)),
        "kb_save",
        serde_json::json!({"kb": "recipes", "path": "a.md", "text": "x", "about": "家常菜"}),
    )
    .await;
    assert_eq!(
        *port.saved.lock().unwrap_or_else(PoisonError::into_inner),
        [KbSave {
            kb: "recipes".to_string(),
            path: "a.md".to_string(),
            text: "x".to_string(),
            append: false,
            about: Some("家常菜".to_string()),
        }],
        "不写 append 是新建"
    );
}

#[tokio::test]
async fn editing_and_deleting_say_what_they_did_and_report_it() {
    let port = Arc::new(Fake {
        written: Some(written(Some("猫\n"), "狗\n", false)),
        ..Fake::default()
    });
    let done = call(
        Some(Arc::clone(&port)),
        "kb_edit",
        serde_json::json!({"kb": "notes", "path": "a.md", "old": "猫", "new": "狗"}),
    )
    .await;
    assert_eq!(text(&done), "Edited a.md in notes.");
    assert_eq!(said(&done), "software/knowledge/kb_edit/edited");
    assert_eq!(done.effects.len(), 1);
    assert_eq!(
        *port.edited.lock().unwrap_or_else(PoisonError::into_inner),
        [(
            "notes".to_string(),
            "a.md".to_string(),
            "猫".to_string(),
            "狗".to_string()
        )]
    );

    let port = Arc::new(Fake::default());
    let done = call(
        Some(Arc::clone(&port)),
        "kb_delete",
        serde_json::json!({"kb": "notes", "path": "old.md"}),
    )
    .await;
    assert_eq!(text(&done), "Moved old.md in notes to the trash.");
    assert_eq!(said(&done), "software/knowledge/kb_delete/trashed");
    assert_eq!(
        done.effects,
        [Effect::Trashed {
            path: PathBuf::from("/kb/notes/old.md"),
            trash: "/trash/old.md".to_string(),
        }],
        "撤销照它移回来"
    );
}

#[tokio::test]
async fn writing_refusals_are_said_plainly() {
    for (refused, name, args, said_key, line) in [
        (
            KbRefused::Exists("notes".to_string(), "a.md".to_string()),
            "kb_save",
            serde_json::json!({"kb": "notes", "path": "a.md", "text": "x"}),
            "common/exists",
            "a.md already exists in notes. append and kb_edit change it.",
        ),
        (
            KbRefused::TooLarge("a.md".to_string()),
            "kb_save",
            serde_json::json!({"kb": "notes", "path": "a.md", "text": "x", "append": true}),
            "common/too-large",
            "a.md would be too large.",
        ),
        (
            KbRefused::NoMatch("a.md".to_string()),
            "kb_edit",
            serde_json::json!({"kb": "notes", "path": "a.md", "old": "猫", "new": "狗"}),
            "common/no-match",
            "The text to replace is not in a.md.",
        ),
        (
            KbRefused::Ambiguous("a.md".to_string(), 2),
            "kb_edit",
            serde_json::json!({"kb": "notes", "path": "a.md", "old": "猫", "new": "狗"}),
            "common/ambiguous",
            "The text to replace appears 2 times in a.md. Include more around it.",
        ),
        (
            KbRefused::Trash("a.md".to_string(), "no trash on this disk".to_string()),
            "kb_delete",
            serde_json::json!({"kb": "notes", "path": "a.md"}),
            "common/trash",
            "Could not move a.md to the trash: no trash on this disk.",
        ),
        (
            KbRefused::BadId("个人备忘".to_string()),
            "kb_save",
            serde_json::json!({"kb": "个人备忘", "path": "a.md", "text": "x"}),
            "common/bad-id",
            "个人备忘 is not a valid knowledge base id (lowercase letters, digits, - and _, starting with a letter).",
        ),
        (
            KbRefused::ReadOnly("notes".to_string()),
            "kb_edit",
            serde_json::json!({"kb": "notes", "path": "a.md", "old": "猫", "new": "狗"}),
            "common/read-only",
            "notes is a mounted folder and read-only.",
        ),
        (
            KbRefused::NotText("cat.pdf".to_string()),
            "kb_save",
            serde_json::json!({"kb": "notes", "path": "cat.pdf", "text": "x"}),
            "common/not-text",
            "cat.pdf is not a text document. Only .md, .markdown and .txt can be written.",
        ),
        (
            KbRefused::NoBase("secret".to_string()),
            "kb_delete",
            serde_json::json!({"kb": "secret", "path": "a.md"}),
            "common/no-base",
            "There is no knowledge base secret.",
        ),
    ] {
        let port = Arc::new(Fake {
            refuse: Some(refused),
            ..Fake::default()
        });
        let done = call(Some(port), name, args).await;
        assert!(done.error, "{name}");
        assert_eq!(text(&done), line, "{name}");
        assert_eq!(said(&done), format!("software/knowledge/{said_key}"));
        assert!(done.effects.is_empty(), "没写成的不报效果");
    }
    for name in ["kb_save", "kb_edit", "kb_delete"] {
        let done = call(
            None,
            name,
            serde_json::json!({"kb": "notes", "path": "a.md", "text": "x", "old": "a", "new": "b"}),
        )
        .await;
        assert_eq!(
            text(&done),
            "Knowledge bases are not available in this session.",
            "{name}"
        );
    }
}
