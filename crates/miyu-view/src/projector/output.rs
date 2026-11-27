//! 一步工具结果原文的前面一段（施工 9-8 十四补；施工 ED-3 从 `tools.rs` 挪来，那一份快满了）：条目的 `output`，供应商侧工具的
//! 结果也照它切。

use miyu_kernel::block::Block as Content;

/// 条目里的结果原文最多几行、几个字节（施工 9-8 十四补）：一页约 1 MiB，一步 8 KiB 放得下上百步。
const OUTPUT_LINES: usize = 100;
const OUTPUT_BYTES: usize = 8 * 1024;

/// 结果原文的前面一段：文字块照先后、块之间空一行（同 `view.detail` 的 `output`），照 [`output_of`] 切。
pub(crate) fn output_head(blocks: &[Content]) -> (Option<String>, bool) {
    let whole = blocks
        .iter()
        .filter_map(|block| match block {
            Content::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<Vec<&str>>()
        .join("\n\n");
    output_of(&whole)
}

/// 一段结果原文的前面一段（供应商侧工具的结果也照它切，施工 HT-3）：最多 [`OUTPUT_LINES`] 行、[`OUTPUT_BYTES`] 字节，
/// 先到的算，切在字的边界上；后面只剩空白的不算切。交回那一段和切没切，没有字的是 `None`。
pub(crate) fn output_of(whole: &str) -> (Option<String>, bool) {
    if whole.is_empty() {
        return (None, false);
    }
    let mut end = whole
        .match_indices('\n')
        .nth(OUTPUT_LINES - 1)
        .map_or(whole.len(), |(at, _)| at);
    if end > OUTPUT_BYTES {
        end = OUTPUT_BYTES;
        while !whole.is_char_boundary(end) {
            end -= 1;
        }
    }
    if whole[end..].trim_end().is_empty() {
        return (Some(whole.to_string()), false);
    }
    (Some(whole[..end].to_string()), true)
}
