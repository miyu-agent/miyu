//! 「AI 怎么认识你」（施工 P-9、P-9 补，`docs/blueprint/prompts.md`「关于你」）：这个人写的称呼、更多描述、回复语言，造会话时照模板
//! `core/profile.txt` 拼成一块，接在人设、场所说明后面（`26-提示词.md` 第四节），记进快照，会话中途不改。

use crate::Snapshot;
use crate::compose::system;

/// 称呼最多几个字：多的截掉（配置那一项也照它限）。
pub const NAME_MAX: usize = 64;

/// 更多描述最多几个字。
pub const ABOUT_MAX: usize = 2000;

/// 回复语言最多几个字（施工 P-9 补，配置那一项也照它限）。
pub const LANGUAGE_MAX: usize = 64;

/// 这个人写的三项：都可以空。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profile {
    /// AI 怎么称呼你（`profile.name`）。
    pub name: String,
    /// 更多描述（`profile.about`）。
    pub about: String,
    /// 希望她默认用哪种语言回（`profile.preferred_reply_language`，施工 P-9 补，2026-10-16 项目主人定）：只是默认，人明说
    /// 换一种就换，所以叫 preferred；模板里只这一行，不另写指令。
    pub preferred_reply_language: String,
}

impl Profile {
    /// 照模板 `template` 拼成一块：几项去掉首尾空白、截到上限，称呼、回复语言里的换行换成空格；空的那一项所在的行（写着
    /// `{name}`、`{about}`、`{preferred_reply_language}` 的那一行）不写；都空的没有。没填回复语言的，拼出来和模板里没有那一行
    /// 时一字不差（施工 P-9 补：以前的请求不变）。原文不转义：是本人写的，同人设（群聊不带，`venues.md`）。
    pub fn block(&self, template: &str) -> Option<String> {
        let name = one_line(clip(self.name.trim(), NAME_MAX));
        let about = clip(self.about.trim(), ABOUT_MAX);
        let language = one_line(clip(self.preferred_reply_language.trim(), LANGUAGE_MAX));
        if name.is_empty() && about.is_empty() && language.is_empty() {
            return None;
        }
        let fields = [
            ("{name}", name.as_str()),
            ("{about}", about),
            ("{preferred_reply_language}", language.as_str()),
        ];
        let lines: Vec<String> = template
            .lines()
            .filter(|line| {
                !fields
                    .iter()
                    .any(|(slot, value)| line.contains(slot) && value.is_empty())
            })
            .map(|line| {
                fields.iter().fold(line.to_string(), |line, (slot, value)| {
                    line.replace(slot, value)
                })
            })
            .collect();
        Some(lines.join("\n"))
    }
}

/// 换行换成空格：称呼、回复语言都只占一行。
fn one_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}

/// 截到 `max` 个字（照字符数，不截半个字）。
fn clip(text: &str, max: usize) -> &str {
    match text.char_indices().nth(max) {
        Some((at, _)) => &text[..at],
        None => text,
    }
}

impl Snapshot {
    /// 带上「关于你」那一块（施工 P-9）：接在人设、场所说明后面，在 [`Snapshot::with_venue`]、群的格式说明以后、
    /// [`Snapshot::with_core_lines`] 以前调；记进快照。没有的 system 一字不变。
    #[must_use]
    pub fn with_profile(mut self, block: Option<String>) -> Snapshot {
        if let Some(block) = &block {
            self.system = system(&[&self.system, block]);
        }
        self.profile = block;
        self
    }
}

#[cfg(test)]
mod tests;
