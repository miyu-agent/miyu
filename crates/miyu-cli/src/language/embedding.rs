//! `miyu check` 合计后面那一段：语义模型的状态（施工 R-25，`docs/blueprint/cli/check.md`）。照核心 `check` 回应的 `embedding`
//! 写：用哪个模型、补着没有，记忆、以前的对话、知识库各补到了哪；关着的、没装的说为什么，没装的另起一行提示怎么装（CL-2 下
//! 照设计 47）。没有的那几样不写。

use serde_json::Value;

use super::Language;

/// 无人格的会话那一间的编号（`miyu_store::recall::NO_PERSONA`）。
const NO_PERSONA: &str = "_none";

impl Language {
    /// 照 `embedding` 写的几行：头一行说模型，后面一类一行、缩两格。
    pub(crate) fn embedding(&self, embedding: &Value) -> Vec<String> {
        let chinese = *self == Language::Chinese;
        let Some(model) = embedding["model"].as_str() else {
            let lines: &[&str] = match (embedding["why"].as_str(), chinese) {
                (Some("off"), true) => {
                    &["语义模型：已关闭（models.embedding = \"off\"），仅按关键词搜索"]
                }
                (Some("off"), false) => {
                    &["Semantic model: off (models.embedding = \"off\"), keyword search only"]
                }
                (_, true) => &[
                    "语义模型：未安装，仅按关键词搜索",
                    "提示：用 'miyu pkg install miyu-embed' 安装",
                ],
                (_, false) => &[
                    "Semantic model: not installed, keyword search only",
                    "hint: install it with 'miyu pkg install miyu-embed'",
                ],
            };
            return lines.iter().map(ToString::to_string).collect();
        };
        let filling = embedding["filling"] == true;
        let mut lines = vec![match (chinese, filling) {
            (true, true) => format!("语义模型：{model}，正在建立索引"),
            (true, false) => format!("语义模型：{model}"),
            (false, true) => format!("Semantic model: {model}, indexing"),
            (false, false) => format!("Semantic model: {model}"),
        }];
        let kinds = [
            ("memory", "persona", "记忆", "Memory"),
            ("turns", "persona", "历史对话", "Past conversations"),
            ("kb", "name", "知识库", "Knowledge bases"),
        ];
        for (key, label, zh, en) in kinds {
            let parts: Vec<String> = embedding[key]
                .as_array()
                .into_iter()
                .flatten()
                .map(|one| {
                    let name = match one[label].as_str().unwrap_or_default() {
                        NO_PERSONA if chinese => "无人格",
                        NO_PERSONA => "no persona",
                        name => name,
                    };
                    format!("{name} {}/{}", one["have"], one["total"])
                })
                .collect();
            if parts.is_empty() {
                continue;
            }
            lines.push(match chinese {
                true => format!("  {zh}：{}", parts.join("、")),
                false => format!("  {en}: {}", parts.join(", ")),
            });
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Language;

    #[test]
    fn each_kind_gets_a_line_and_empty_ones_none() {
        let embedding = json!({
            "model": "local:granite",
            "filling": true,
            "memory": [{"persona": "engineer", "have": 120, "total": 120}, {"persona": "_none", "have": 1, "total": 2}],
            "turns": [],
            "kb": [{"id": "world", "name": "设定集", "have": 840, "total": 900}, {"id": "notes", "name": "notes", "have": 12, "total": 12}],
        });
        assert_eq!(
            Language::Chinese.embedding(&embedding),
            [
                "语义模型：local:granite，正在建立索引",
                "  记忆：engineer 120/120、无人格 1/2",
                "  知识库：设定集 840/900、notes 12/12",
            ]
        );
        let mut done = embedding.clone();
        done["filling"] = json!(false);
        assert_eq!(
            Language::English.embedding(&done),
            [
                "Semantic model: local:granite",
                "  Memory: engineer 120/120, no persona 1/2",
                "  Knowledge bases: 设定集 840/900, notes 12/12",
            ]
        );
    }

    #[test]
    fn off_and_not_installed_say_what_to_do() {
        let off = json!({"model": null, "why": "off"});
        assert_eq!(
            Language::Chinese.embedding(&off),
            ["语义模型：已关闭（models.embedding = \"off\"），仅按关键词搜索"]
        );
        let missing = json!({"model": null, "why": "not_installed"});
        assert_eq!(
            Language::English.embedding(&missing),
            [
                "Semantic model: not installed, keyword search only",
                "hint: install it with 'miyu pkg install miyu-embed'",
            ]
        );
    }
}
