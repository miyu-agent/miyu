//! 碰到数据根里知识库的文档（施工 K-3 补）：照样拒，话换成指她去用知识库的工具，给人看的说法是 `core/permissions/forbidden-kb`；
//! 数据根里别的地方照旧是「Miyu 自己的数据」。

use super::*;

#[tokio::test]
async fn a_knowledge_base_document_is_refused_with_the_kb_tools_named() {
    let home = Home::outside_temp();
    let doc = home.root.path().join("kb").join("notes");
    std::fs::create_dir_all(&doc).expect("建得了");
    let doc = doc.join("todo.md");
    std::fs::write(&doc, "x").expect("写得进");
    let doc = doc.to_string_lossy().into_owned();
    let kit = Kit::new();
    let script = Script::new([one("edit", &doc), Play::Says("好。")]);
    let handle = session(&home, &script, &kit, Level::Full, false, true).await;
    turn(&handle).await;
    let (result, by) = first_result(&home.log(handle.id()));
    assert_eq!(result.status, ToolStatus::Denied);
    assert!(by_permissions(&by), "{by:?}");
    assert!(text(&result).contains("kb_delete"), "{}", text(&result));
    assert_eq!(
        result.human,
        Some(Said::new("core/permissions/forbidden-kb").with("path", doc.as_str()))
    );
    assert!(kit.edit.calls().is_empty());
}
