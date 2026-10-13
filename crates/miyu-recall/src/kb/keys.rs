//! 小节的关键词（施工 K-9，`docs/blueprint/knowledge.md`「对外的样子」、「怎么走」第九条；19 N9，2026-10-14 项目主人定）：一节
//! 标题下面正文的头一行写 `关键词：守灯人、灯塔的姑娘、Eileen`，每个是这一节的一个别名，同标题名触发自动想起，那一行不进带上的
//! 字。认哪几种开头、冒号、分隔照 `resources/core/knowledge/keys.toml`（数据）。

use serde::Deserialize;

/// 关键词那一行的写法：认哪几种开头、冒号，用什么分开。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Keys {
    /// 那一行照哪几种开头（不分大小写）。
    labels: Vec<String>,
    /// 开头后面接哪几种冒号。
    colons: Vec<String>,
    /// 关键词之间用哪几种符号分开。
    separators: Vec<String>,
}

impl Keys {
    /// 读写法表（`core/knowledge/keys.toml`）。
    ///
    /// # Errors
    ///
    /// 不是合写法的 TOML、少了格、写了别的格、哪一格是空的或者有空字符串：英文的一句为什么。
    pub fn parse(text: &str) -> Result<Keys, String> {
        let keys: Keys =
            toml_edit::de::from_str(text).map_err(|error| error.to_string().trim().to_string())?;
        for (name, list) in [
            ("labels", &keys.labels),
            ("colons", &keys.colons),
            ("separators", &keys.separators),
        ] {
            if list.is_empty() || list.iter().any(String::is_empty) {
                return Err(format!("{name} must list non-empty strings"));
            }
        }
        Ok(keys)
    }

    /// 一节正文 `text`（不带标题那一行）的关键词：头一行（空行不算）是关键词那一行的，交回分开的关键词（去掉前后空白，空的
    /// 不要）和那一行以后的正文；不是的交回没有关键词和原样的正文。
    #[must_use]
    pub fn split<'a>(&self, text: &'a str) -> (Vec<String>, &'a str) {
        let start = text.len() - text.trim_start().len();
        let (line, rest) = match text[start..].split_once('\n') {
            Some((line, rest)) => (line, rest),
            None => (&text[start..], ""),
        };
        let Some(listed) = self.listed(line.trim()) else {
            return (Vec::new(), text);
        };
        let mut keys = vec![listed.to_string()];
        for separator in &self.separators {
            keys = keys
                .iter()
                .flat_map(|key| key.split(separator.as_str()))
                .map(str::to_string)
                .collect();
        }
        let keys = keys
            .into_iter()
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
            .collect();
        (keys, rest)
    }

    /// 一行 `line` 是关键词那一行的，交回冒号后面那一截。
    fn listed<'a>(&self, line: &'a str) -> Option<&'a str> {
        self.labels.iter().find_map(|label| {
            let length = line
                .char_indices()
                .nth(label.chars().count())
                .map_or(line.len(), |(at, _)| at);
            if line[..length].to_lowercase() != label.to_lowercase() {
                return None;
            }
            let after = line[length..].trim_start();
            self.colons
                .iter()
                .find_map(|colon| after.strip_prefix(colon.as_str()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 出厂的那一份写法表。
    fn keys() -> Keys {
        Keys::parse(include_str!(
            "../../../../resources/core/knowledge/keys.toml"
        ))
        .expect("合写法")
    }

    #[test]
    fn the_line_under_the_heading_lists_aliases() {
        let keys = keys();
        assert_eq!(
            keys.split("关键词：守灯人、灯塔的姑娘、Eileen\n\n守灯人，十九岁。"),
            (
                vec!["守灯人".into(), "灯塔的姑娘".into(), "Eileen".into()],
                "\n守灯人，十九岁。"
            )
        );
        assert_eq!(
            keys.split("\n\nKeys: Eileen, lighthouse girl ; keeper\n正文"),
            (
                vec!["Eileen".into(), "lighthouse girl".into(), "keeper".into()],
                "正文"
            ),
            "空行不算头一行；开头不分大小写；英文冒号、逗号、分号"
        );
        assert_eq!(
            keys.split("キーワード：アイリーン，灯台守；守灯人"),
            (
                vec!["アイリーン".into(), "灯台守".into(), "守灯人".into()],
                ""
            ),
            "只有这一行的，正文是空的"
        );
        assert_eq!(
            keys.split("关键词 ： 、 ,艾琳,,\n正文"),
            (vec!["艾琳".into()], "正文"),
            "冒号前后的空白不碍事，空的不要"
        );
        assert_eq!(
            keys.split("关键词：\n正文"),
            (Vec::new(), "正文"),
            "只写了开头的照样去掉"
        );
    }

    #[test]
    fn other_lines_are_not_keys() {
        let keys = keys();
        for text in [
            "守灯人，十九岁。\n关键词：艾琳",
            "关键词很重要。",
            "关键词艾琳",
            "keystone: 拱顶石",
            "",
        ] {
            assert_eq!(keys.split(text), (Vec::new(), text), "{text}");
        }
    }

    #[test]
    fn a_bad_table_is_refused() {
        assert!(
            Keys::parse("labels = [\"keys\"]\ncolons = [\":\"]\n").is_err(),
            "少了格"
        );
        assert!(
            Keys::parse("labels = []\ncolons = [\":\"]\nseparators = [\",\"]\n").is_err(),
            "空的"
        );
        assert!(
            Keys::parse("labels = [\"\"]\ncolons = [\":\"]\nseparators = [\",\"]\n").is_err(),
            "空字符串"
        );
        assert!(
            Keys::parse("labels = [\"k\"]\ncolons = [\":\"]\nseparators = [\",\"]\nmore = 1\n")
                .is_err(),
            "别的格"
        );
    }
}
