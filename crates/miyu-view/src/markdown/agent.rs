//! `agent` 预设的旁白、占位（施工 SE-1 中下下，core1 2026-11-04 定）：一律英文，不跟 `locale`。它是给别的模型读的机械文本，
//! 同开头那一段（[`super::AGENT_NOTE`]）写成渲染器里的常量，不放进给人看的字：放进资源会被当成要翻译的一句，也会跟着
//! 人看的英文改。人说的话、她回的话照原文，不在这里。
//!
//! 编号同给人看的字（`core/` 后面那一截，`export/compacted`、`view/summary/thought-for`），字段照 `{名字}` 换，字段里的控制
//! 字符换成 `�`（同 `miyu_store::human::clean`）。这里没有的编号（只有 `read` 用得到的）照英文那一份给人看的字。

/// 编号到英文：`agent` 走得到的每一句。
const WORDS: &[(&str, &str)] = &[
    ("export/thinking", "Thinking"),
    ("view/summary/thought-for", "Thought for {elapsed}"),
    ("export/output", "Output ({count} lines)"),
    ("export/output/one", "Output ({count} line)"),
    ("export/image", "[Image: {about}]"),
    ("export/image/bare", "[Image]"),
    ("export/file", "[File: {about}]"),
    ("export/file/bare", "[File]"),
    ("export/compacted", "Context compacted"),
    ("export/compaction-failed", "Compaction failed: {why}"),
    ("export/summary", "Summary"),
    ("export/cleared", "Context cleared"),
    ("export/paused", "Auto-compaction paused"),
    ("export/job/done", "Background command finished: {title}"),
    ("export/job/failed", "Background command failed: {title}"),
    ("export/job/stopped", "Background command stopped: {title}"),
    (
        "export/agent/stopped",
        "The subagent stopped before finishing",
    ),
    ("export/lines", "{title} printed {count} lines"),
    ("export/lines/one", "{title} printed {count} line"),
    ("export/answered", "Questions answered"),
    ("export/recap", "Recap"),
    ("export/model", "Model changed to {model}"),
    ("export/persona", "Persona changed to {to}"),
    ("export/preset", "Preset changed to {to}"),
    ("export/workspace", "Working directory changed to {cwd}"),
    ("export/peer", "Session {session}: {reason}"),
    ("export/end/error", "The turn ended with an error: {why}"),
    ("export/end/interrupted", "The turn was interrupted"),
    ("export/end/step-limit", "The turn hit the step limit"),
    (
        "export/end/aborted",
        "The turn was cut off when Miyu stopped",
    ),
];

/// 编号 `key`（带 `core/` 的全名）照字段说一句；这里没有的是 `None`。
pub(super) fn say(key: &str, fields: &[(&str, String)]) -> Option<String> {
    let key = key.strip_prefix("core/").unwrap_or(key);
    let (_, template) = WORDS.iter().find(|(name, _)| *name == key)?;
    // 一遍过：字段的值里写着 `{别的字段}` 的不再换。
    let mut said = String::with_capacity(template.len());
    let mut rest = *template;
    while let Some(open) = rest.find('{') {
        said.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find('}')?;
        let (_, value) = fields.iter().find(|(name, _)| *name == &after[..close])?;
        said.extend(value.chars().map(|c| match c.is_control() {
            true => '\u{FFFD}',
            false => c,
        }));
        rest = &after[close + 1..];
    }
    said.push_str(rest);
    Some(said)
}

#[cfg(test)]
mod tests {
    use super::{WORDS, say};

    #[test]
    fn every_line_is_english_and_fills_its_fields() {
        for (key, template) in WORDS {
            assert!(template.is_ascii(), "{key}：{template}");
        }
        assert_eq!(
            say("core/export/end/error", &[("why", "a\nb".to_string())]).as_deref(),
            Some("The turn ended with an error: a\u{FFFD}b")
        );
        assert_eq!(
            say(
                "core/export/model",
                &[("model", "{model}".to_string()), ("x", String::new())]
            )
            .as_deref(),
            Some("Model changed to {model}"),
            "值里的花括号不再换"
        );
        assert_eq!(say("core/export/peer", &[]), None, "少了字段的没有");
        assert_eq!(
            say("core/export/head/time", &[]),
            None,
            "只有 read 用的不在这里"
        );
    }
}
