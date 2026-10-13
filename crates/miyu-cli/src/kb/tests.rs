//! `miyu kb` 印的样子（`docs/blueprint/cli/kb.md`「样子」）：库一行一个、编号对齐、名字和编号一样的不写、用不了的接上为什么；
//! 搜到的一块一段、PDF 说第几页；读的不止一页的末尾说第几页。

use serde_json::json;

use super::shown::{bases, hits, page};
use crate::language::Language;

#[test]
fn bases_one_line_each() {
    let listed = [
        json!({"id": "notes", "name": "日语笔记", "description": "学日语的", "documents": 12, "chunks": 40}),
        json!({"id": "world", "name": "world", "description": "", "documents": 1, "chunks": 3}),
        json!({"id": "usb", "name": "U 盘", "description": "", "documents": 3, "chunks": 9, "error": "mounted folder /mnt/usb: not found"}),
    ];
    assert_eq!(
        bases(&listed, &Language::Chinese),
        [
            "notes  日语笔记  12 篇  学日语的",
            "world  1 篇",
            "usb    U 盘  3 篇（用不了：mounted folder /mnt/usb: not found）",
        ]
    );
    assert_eq!(
        bases(&listed[1..2], &Language::English),
        ["world  1 document"]
    );
    assert_eq!(
        bases(&listed[0..1], &Language::English),
        ["notes  日语笔记  12 documents  学日语的"]
    );
}

#[test]
fn hits_one_paragraph_each() {
    let found = [
        json!({"kb": "notes", "path": "grammar.md", "heading": "语法 › て形", "text": "食べる → 食べて\n\n例句\n"}),
        json!({"kb": "manuals", "path": "cat.pdf", "heading": "", "page": 3, "text": "每年十月打疫苗。"}),
        json!({"kb": "world", "path": "east.txt", "heading": "", "text": "东边的镇子靠海。"}),
    ];
    assert_eq!(
        hits(&found, &Language::Chinese),
        "notes  grammar.md  语法 › て形\n  食べる → 食べて\n\n  例句\n\nmanuals  cat.pdf  第 3 页\n  每年十月打疫苗。\n\nworld  east.txt\n  东边的镇子靠海。"
    );
    assert_eq!(
        hits(&found[1..2], &Language::English),
        "manuals  cat.pdf  page 3\n  每年十月打疫苗。"
    );
}

#[test]
fn a_page_says_which_when_there_are_more() {
    let one = json!({"text": "全文\n", "page": 1, "pages": 1});
    assert_eq!(page(&one, &Language::Chinese), "全文");
    let two = json!({"text": "第二页\n", "page": 2, "pages": 3});
    assert_eq!(page(&two, &Language::Chinese), "第二页\n（第 2/3 页）");
    assert_eq!(page(&two, &Language::English), "第二页\n(page 2 of 3)");
}

#[test]
fn an_auto_base_says_so_after_its_count() {
    // 施工 K-6 补：开了自动想起的，篇数后面接「自动想起」，再接说明。
    let listed = [
        json!({"id": "world", "name": "雾港", "description": "世界观", "documents": 4, "chunks": 17, "auto": true}),
        json!({"id": "notes", "name": "notes", "description": "", "documents": 1, "chunks": 1, "auto": false}),
    ];
    assert_eq!(
        bases(&listed, &Language::Chinese),
        ["world  雾港  4 篇  自动想起  世界观", "notes  1 篇"]
    );
    assert_eq!(
        bases(&listed[0..1], &Language::English),
        ["world  雾港  4 documents  auto recall  世界观"]
    );
}

#[test]
fn plain_refusals_in_both_languages() {
    // 施工 K-7 修：不认的格式、编号写错、页码出界，中英文。
    let (zh, en) = (Language::Chinese, Language::English);
    assert_eq!(
        en.kb_not_document("report.docx"),
        "Only .md, .markdown, .txt, .pdf, .html, .htm can be added: report.docx"
    );
    assert_eq!(
        zh.kb_not_document("report.docx"),
        "只收 .md、.markdown、.txt、.pdf、.html、.htm：report.docx"
    );
    assert!(en.kb_bad_id("日语").ends_with("at most 64: 日语"));
    assert_eq!(zh.kb_no_page(9, 3), "没有第 9 页，一共 3 页。");
    assert_eq!(en.kb_no_page(9, 1), "There is no page 9; it has 1 page.");
    assert_eq!(en.kb_no_page(9, 3), "There is no page 9; it has 3 pages.");
}
