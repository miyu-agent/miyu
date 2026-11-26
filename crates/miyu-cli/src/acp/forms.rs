//! 她问人的几道题（`ask_user`）和规范的表单（`elicitation/create` 的 `form` 一种）之间来回换（设计 73 第五节第 4 条）。照
//! LiteMiyu `litemiyu acp` 的写法：表单是一层平的对象，第 N 道题是属性 `qN`；有选项的题另有一个 `qN_other` 给人自己写。
//! 单选的是带 `enum` 的字符串，多选的是元素带 `enum` 的数组，没有选项的是一个字符串。

use serde_json::{Map, Value, json};

use miyu_kernel::event::Question;

use crate::language::Language;

/// 第 `index` 道题（从 0 数）的属性名。
fn name(index: usize) -> String {
    format!("q{}", index + 1)
}

/// 有选项的题给人自己写的那一格的属性名。
fn other(index: usize) -> String {
    format!("q{}_other", index + 1)
}

/// 表单上面的那句话：只有一道题的就是那道题，几道的是一句总的。
pub(crate) fn message(language: Language, questions: &[Question]) -> String {
    match questions {
        [only] => only.question.clone(),
        _ => language.acp_questions().to_string(),
    }
}

/// 这几道题的表单（`requestedSchema`）。题目只写一处（真 Zed 里表单的标题、格的标题、格的说明各写一遍，同一句出现三次）：
/// 一道题的，题目是表单上面那句话，格的标题只写 `header`（没有的不写）；几道的，格的标题是题目，有 `header` 的写成
/// `<header>：<题目>`。都不写说明。给人自己写的那一格标题只写「其他」，紧跟在那道题后面。
pub(crate) fn schema(language: Language, questions: &[Question]) -> Value {
    let mut properties = Map::new();
    let single = questions.len() == 1;
    for (index, question) in questions.iter().enumerate() {
        let title = match (single, &question.header) {
            (true, header) => header.clone(),
            (false, Some(header)) => Some(format!("{header}：{}", question.question)),
            (false, None) => Some(question.question.clone()),
        };
        let labels: Vec<&str> = question
            .options
            .iter()
            .map(|choice| choice.label.as_str())
            .collect();
        let mut field = match (labels.is_empty(), question.multiple) {
            (true, _) => json!({"type": "string"}),
            (false, false) => json!({"type": "string", "enum": labels}),
            (false, true) => json!({"type": "array", "items": {"type": "string", "enum": labels}}),
        };
        if let Some(title) = title {
            field["title"] = json!(title);
        }
        properties.insert(name(index), field);
        if !labels.is_empty() {
            let field = json!({"type": "string", "title": language.acp_other()});
            properties.insert(other(index), field);
        }
    }
    json!({"type": "object", "properties": properties})
}

/// 每道题都没答（人没填就关了、回的读不懂）：`session.answer` 的 `answers`，一题一个空的。
pub(crate) fn unanswered(questions: &[Question]) -> Vec<Value> {
    questions.iter().map(|_| json!({})).collect()
}

/// 把编辑器交回的表单内容（`content`）读成每道题的回答（`{"picked", "text"}`）。选的不是那道题的选项的不算；空白的字不算。
/// 对不上题目的整组（单选的选了几项）由核心拒（`bad_answer`），这边再照每道都没答交一次。
pub(crate) fn answers(questions: &[Question], content: &Value) -> Vec<Value> {
    questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            let offered = |label: &str| question.options.iter().any(|choice| choice.label == label);
            let mut picked: Vec<String> = Vec::new();
            let mut text = None;
            match &content[name(index)] {
                Value::String(written) if question.options.is_empty() => {
                    text = Some(written.clone()).filter(|text| !text.trim().is_empty());
                }
                Value::String(label) if offered(label) => picked.push(label.clone()),
                Value::Array(labels) => {
                    for label in labels.iter().filter_map(Value::as_str) {
                        if offered(label) && !picked.iter().any(|had| had == label) {
                            picked.push(label.to_string());
                        }
                    }
                }
                _ => {}
            }
            if let Some(written) = content[other(index)].as_str()
                && !written.trim().is_empty()
            {
                text = Some(written.to_string());
            }
            let mut answer = json!({});
            if !picked.is_empty() {
                answer["picked"] = json!(picked);
            }
            if let Some(text) = text {
                answer["text"] = json!(text);
            }
            answer
        })
        .collect()
}

#[cfg(test)]
mod tests;
