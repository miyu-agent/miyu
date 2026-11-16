//! 改数据根里知识库的文档（施工 K-3 补）：照样拒，话换成指她去用知识库的工具，给人看的说法是 `core/permissions/forbidden-kb`；
//! 数据根里别的地方照旧是「Miyu 自己的数据」。读照数据根别处，放行（施工 RO-2）。

use super::*;

#[tokio::test]
async fn a_knowledge_base_document_is_refused_with_the_kb_tools_named() {
    let home = Home::outside_temp();
    let doc = home.root.knowledge().join("notes");
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
    // 路径照模板转义换进去（Windows 上反斜杠写成两个），同生产的写法拼。
    let path = miyu_kernel::template::escape(&doc);
    assert_eq!(
        text(&result),
        format!(
            "\"{path}\" is a knowledge base document. Change it with the knowledge base tools: kb_edit, kb_delete.\n"
        )
    );
    assert_eq!(
        result.human,
        Some(Said::new("core/permissions/forbidden-kb").with("path", doc.as_str()))
    );
    assert!(kit.edit.calls().is_empty());
}

#[tokio::test]
async fn a_knowledge_base_document_reads_like_any_file() {
    let home = Home::outside_temp();
    let doc = home.root.knowledge().join("notes");
    std::fs::create_dir_all(&doc).expect("建得了");
    let doc = doc.join("todo.md");
    std::fs::write(&doc, "x").expect("写得进");
    let kit = Kit::new();
    let script = Script::new([one("read", &doc.to_string_lossy()), Play::Says("好。")]);
    let handle = session(&home, &script, &kit, Level::Full, true, true).await;
    turn(&handle).await;
    let (result, _) = first_result(&home.log(handle.id()));
    assert_eq!(result.status, ToolStatus::Ok, "{}", text(&result));
    assert_eq!(kit.read.calls().len(), 1);
}
