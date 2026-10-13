//! 知识库那一层的错写成哪一种拒绝（施工 K-7 上）：真核心上挪不进回收站碰不出来，这里照每一种错直接对。核心照资源目录交
//! 知识库搜索的下限（施工 K-2 修）。

use miyu_store::knowledge::KnowledgeError;
use miyu_store::resources::ResourceRoot;

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
        (KnowledgeError::Packaged, "kb_packaged", None),
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
        // 每一种都有给人看的话（施工 K-7 下：K-7 上加的五种头一回漏了，头和命令行只看得到「已拒绝。」）。
        assert_ne!(
            refused.message(crate::refusal::Locale::Zh),
            "已拒绝。",
            "{reason}"
        );
        assert_ne!(
            refused.message(crate::refusal::Locale::En),
            "Refused.",
            "{reason}"
        );
    }
}

#[test]
fn the_core_takes_its_search_floors_from_resources() {
    // 施工 K-2 修：出厂的表里 bge 是 0.50，表里没有的照 0.40；表读不出来的都照 0.40（记一行 WARN）。
    let (root, dir) = crate::test_support::temp_root("kb-floor");
    let shipped = ResourceRoot::at(crate::test_support::resources());
    let knowledge = super::shelf(&root, None, None, &shipped, &[]);
    assert!((knowledge.floor("local:bge-small-zh-v1.5") - 0.50).abs() < f32::EPSILON);
    assert!((knowledge.floor("emb/text-emb") - miyu_recall::FLOOR).abs() < f32::EPSILON);
    let broken = ResourceRoot::at(dir.join("no-resources"));
    let knowledge = super::shelf(&root, None, None, &broken, &[]);
    assert!((knowledge.floor("local:bge-small-zh-v1.5") - miyu_recall::FLOOR).abs() < f32::EPSILON);
    drop(std::fs::remove_dir_all(&dir));
}
