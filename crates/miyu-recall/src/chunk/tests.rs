//! 切块（`knowledge.md`「怎么走」第二条）：Markdown 照标题、标题路径照层级、代码块里的 `#` 不算；纯文本照段落；太长的照段落、
//! 照字再切，不超过上限、截在字的边界上；空白的不要；同样的文档同样的块。

use super::*;

fn headings(chunks: &[Chunk]) -> Vec<&str> {
    chunks.iter().map(|chunk| chunk.heading.as_str()).collect()
}

#[test]
fn markdown_is_cut_at_headings_with_their_path() {
    let source = "开头的一段\n\n# 语法\n\n语法总说\n\n## て形\n\n食べる → 食べて\n\n```\n# 不是标题\n```\n\n## た形\n\n食べた\n\n# 单词\n\n猫 ねこ\n\n# 空的标题\n\n";
    let chunks = markdown(source);
    assert_eq!(
        headings(&chunks),
        ["", "语法", "语法 › て形", "语法 › た形", "单词"],
        "空的一节不要"
    );
    assert_eq!(chunks[0].text, "开头的一段");
    assert_eq!(
        chunks[2].text, "# 语法 › て形\n食べる → 食べて\n\n```\n# 不是标题\n```",
        "标题路径写在头一行，代码块里的 # 不算标题"
    );
    assert_eq!(markdown(source), chunks, "同样的文档同样的块");
}

#[test]
fn a_heading_closes_its_deeper_siblings() {
    let chunks = markdown("# 甲\n\n### 甲三\n\n一\n\n## 乙二\n\n二\n\n# 丙\n\n三\n");
    assert_eq!(headings(&chunks), ["甲 › 甲三", "甲 › 乙二", "丙"]);
}

#[test]
fn plain_text_packs_paragraphs_up_to_the_limit() {
    let paragraph = "字".repeat(100);
    let source = (0..10)
        .map(|_| paragraph.clone())
        .collect::<Vec<_>>()
        .join("\n\n");
    let chunks = plain(&source);
    assert!(chunks.len() > 1, "{}", chunks.len());
    for chunk in &chunks {
        assert!(chunk.text.len() <= LIMIT, "{}", chunk.text.len());
        assert!(chunk.heading.is_empty());
    }
    let joined: String = chunks
        .iter()
        .map(|chunk| chunk.text.replace("\n\n", ""))
        .collect();
    assert_eq!(joined, paragraph.repeat(10), "一个字都不丢、不重");
    assert!(plain("\n\n  \n").is_empty(), "只有空白的不要");
}

#[test]
fn a_paragraph_too_long_is_cut_at_char_boundaries_and_keeps_its_heading() {
    // 标题那一行 20 字节，留给正文的 1180 不是 3 的倍数：照字节截会截在字中间。
    let long = "長".repeat(1000);
    let chunks = markdown(&format!("# 很长的一节 2\n\n{long}\n"));
    assert!(chunks.len() >= 3, "{}", chunks.len());
    let mut body = String::new();
    for chunk in &chunks {
        assert!(chunk.text.len() <= LIMIT, "{}", chunk.text.len());
        assert_eq!(chunk.heading, "很长的一节 2");
        let rest = chunk
            .text
            .strip_prefix("# 很长的一节 2\n")
            .expect("头一行是标题");
        body.push_str(rest);
    }
    assert_eq!(body, long, "截在字的边界上，一个字都不丢");
}
