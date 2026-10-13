//! 知识库那一层的错写成哪一种拒绝（施工 K-7 上）：真核心上挪不进回收站碰不出来，这里照每一种错直接对。

use miyu_store::knowledge::KnowledgeError;

use super::blocking;

#[tokio::test]
async fn each_knowledge_error_becomes_its_refusal() {
    let cases = [
        (KnowledgeError::UnknownBase, "unknown_kb", None),
        (KnowledgeError::UnknownDocument, "unknown_document", None),
        (KnowledgeError::BadPage, "bad_params", None),
        (KnowledgeError::BaseExists, "kb_exists", None),
        (KnowledgeError::Exists, "document_exists", None),
        (KnowledgeError::ReadOnly, "kb_read_only", None),
        (
            KnowledgeError::FolderUnavailable("gone".to_string()),
            "folder_unavailable",
            Some("gone"),
        ),
        (
            KnowledgeError::Trash("no trash on this disk".to_string()),
            "trash_failed",
            Some("no trash on this disk"),
        ),
        (
            KnowledgeError::Io("broken".to_string()),
            "internal_error",
            None,
        ),
    ];
    for (error, reason, message) in cases {
        let refused = blocking(move || Err::<(), _>(error))
            .await
            .expect_err("拒了");
        assert_eq!(refused.reason, reason);
        let said = refused
            .data
            .as_ref()
            .and_then(|data| data.get("message"))
            .and_then(|message| message.as_str());
        assert_eq!(said, message, "{reason}");
    }
}
