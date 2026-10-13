//! 一个知识库的 `kb.toml`（施工 K-1 上，`docs/blueprint/knowledge.md`「对外的样子」）：给人看的名字、一句话说这个库是什么。
//! 几格都可以不写；别的格不认，读进来报错（调的一方照没有 `kb.toml` 的样子用，把为什么交给人看）。`mount` 是挂进来的现成
//! 目录（施工 K-4）：写了的库，文档在那里，只读。账号自己的 `kb.toml` 记软件包带来的库人改过的开关（施工 K-8，[`switches`]）。

use std::collections::BTreeMap;

use serde::Deserialize;

/// `kb.toml` 里写的。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct About {
    /// 给人看的名字；没写的照编号。
    #[serde(default)]
    pub name: Option<String>,
    /// 一句话说这个库是什么；没写的空着。
    #[serde(default)]
    pub description: Option<String>,
    /// 挂进来的目录（施工 K-4）：绝对路径，或者 `~/` 开头；路径照写的原样，换成真实位置是调的一方的事。没写的是自己的库。
    #[serde(default)]
    pub mount: Option<String>,
    /// 自动想起（施工 K-6 下，19 N5）：`true` 的库，人说的话提到了它的某一段，回合开始就带给她。没写的是关（默认关）。
    #[serde(default)]
    pub auto: Option<bool>,
}

/// 读 `kb.toml` 的字。
///
/// # Errors
///
/// 不是合写法的 TOML、写了别的格、格的类型不对：英文的一句为什么。
pub fn about(text: &str) -> Result<About, String> {
    toml_edit::de::from_str(text).map_err(|error| error.to_string().trim().to_string())
}

/// 账号自己的 `kb.toml`（施工 K-8）：`[auto]` 下一行一个软件包带来的库，`"<包>/<编号>" = true/false`，是人改过的自动想起的
/// 开关。别的表不看（以后加的不碍事）。
#[derive(Debug, Default, Deserialize)]
struct Switches {
    #[serde(default)]
    auto: BTreeMap<String, bool>,
}

/// 读账号自己的 `kb.toml` 里人改过的开关：`<包>/<编号>` → 开没开。
///
/// # Errors
///
/// 不是合写法的 TOML、`[auto]` 里的值不是真假：英文的一句为什么。
pub fn switches(text: &str) -> Result<BTreeMap<String, bool>, String> {
    let read: Switches =
        toml_edit::de::from_str(text).map_err(|error| error.to_string().trim().to_string())?;
    Ok(read.auto)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_fields_are_optional_and_others_are_refused() {
        assert_eq!(about("").expect("空的合写法"), About::default());
        assert_eq!(
            about("name = \"日语笔记\"\ndescription = \"学日语的笔记\"\n").expect("合写法"),
            About {
                name: Some("日语笔记".into()),
                description: Some("学日语的笔记".into()),
                mount: None,
                auto: None,
            }
        );
        assert_eq!(about("auto = true\n").expect("合写法").auto, Some(true));
        assert!(about("auto = \"yes\"\n").is_err());
        assert_eq!(
            about("mount = \"~/notes\"\n")
                .expect("合写法")
                .mount
                .as_deref(),
            Some("~/notes")
        );
        assert!(about("title = \"错的格\"\n").is_err());
        // 账号自己的开关（施工 K-8）：别的表不看，值不是真假的读不懂。
        let read =
            switches("[auto]\n\"world-pack/world\" = false\n[later]\nx = 1\n").expect("合写法");
        assert_eq!(read.get("world-pack/world"), Some(&false));
        assert!(switches("").expect("空的合写法").is_empty());
        assert!(switches("[auto]\n\"a/b\" = \"no\"\n").is_err());
        assert!(about("name = 3\n").is_err());
        assert!(about("name = \n").is_err());
    }
}
