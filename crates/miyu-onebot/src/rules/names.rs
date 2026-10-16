//! 桥的工具给人看的名字（施工 S-6 上）：字挪进包文件夹以后核心不再读桥的 `human/`，桥照第三方包的路子在 `provide` 里每件带
//! `display_name`（`providers.md` 的 `provide`，作者说明第四节第 4 条），核心照它进 `human.get`。名字照包文件夹
//! `data/human/<语言>.json` 的 `tools.<名字>.name`，有几种语言的文件就带几种；没有的工具、没有那个目录的不带（读不到的不算错，
//! 同 `Human::load`）。名字后面跟哪一个参数照同一处的 `subject`，`provide` 时带 `subject`，核心一起放进 `human.get`。

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::Path;

use miyu_chat::{Problem, Source};
use miyu_client::settings::problem::Code;
use miyu_client::texts::Human;

use super::files;

/// 给人看的字在数据目录里的哪个子目录。
const HUMAN: &str = "human";

/// 说法编号的前缀：这里只取工具的名字，用不上，照 `texts.rs` 的写。
const KEYS: &str = "software/onebot";

/// 一件工具给人看的样子：每种语言的名字，名字后面跟哪一个参数。
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Face {
    /// 语言到名字。
    pub(super) names: BTreeMap<String, String>,
    /// 名字后面跟哪一个参数的值：照语言的先后，第一份写了的那一个。
    pub(super) subject: Option<String>,
}

/// 每件工具给人看的样子：工具名到 [`Face`]。
pub(super) type Names = BTreeMap<String, Face>;

/// 读数据目录 `data` 里 `tools` 这几件的名字。读得到却读不懂的那一份记一条 `bad_format`（文件照 `human/<语言>.json` 写），
/// 别的语言照读；空白的名字不带（核心整份不收空白的显示名）。
pub(super) fn load(data: &Path, tools: &[&str], problems: &mut Vec<Problem>) -> Names {
    let mut languages: Vec<String> = match std::fs::read_dir(data.join(HUMAN)) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.strip_suffix(".json").map(str::to_string)
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    languages.sort();
    let mut names = Names::new();
    for language in languages {
        let human = match Human::default().with_dir(data, KEYS, &language) {
            Ok(human) => human,
            Err(error) => {
                let file = format!("{HUMAN}/{language}.json");
                problems.push(files::whole(
                    Code::BadFormat,
                    Source::Factory,
                    &file,
                    Some(error.why),
                ));
                continue;
            }
        };
        for tool in tools {
            if let Some(face) = human.tool(tool).filter(|face| !face.name.trim().is_empty()) {
                let kept = names.entry((*tool).to_string()).or_default();
                kept.names.insert(language.clone(), face.name.clone());
                if kept.subject.is_none() {
                    kept.subject.clone_from(&face.subject);
                }
            }
        }
    }
    names
}
