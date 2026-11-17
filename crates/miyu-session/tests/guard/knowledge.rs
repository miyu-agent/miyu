//! 数据根里知识库的文档（施工 K-3 补）：同数据根别处，读哪一级都放行（施工 RO-2），完全放开下文件工具写得进（施工 RO-3：
//! 索引照文件的指纹补，直接改文件不会和索引对不上）。

use super::*;

#[tokio::test]
async fn a_knowledge_base_document_is_written_like_any_file_at_full_access() {
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
    let (result, _) = first_result(&home.log(handle.id()));
    assert_eq!(result.status, ToolStatus::Ok, "{}", text(&result));
    assert_eq!(kit.edit.calls().len(), 1);
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
