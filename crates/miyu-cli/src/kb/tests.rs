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
