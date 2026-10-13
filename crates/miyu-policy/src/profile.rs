//! 「AI 怎么认识你」（施工 P-9，`docs/blueprint/prompts.md`「关于你」）：这个人写的称呼和更多描述，造会话时照模板
//! `core/profile.txt` 拼成一块，接在人设、场所说明后面（`26-提示词.md` 第四节），记进快照，会话中途不改。

use crate::Snapshot;
use crate::compose::system;

/// 称呼最多几个字：多的截掉（配置那一项也照它限）。
pub const NAME_MAX: usize = 64;

/// 更多描述最多几个字。
pub const ABOUT_MAX: usize = 2000;

/// 这个人写的两项：都可以空。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profile {
    /// AI 怎么称呼你（`profile.name`）。
    pub name: String,
    /// 更多描述（`profile.about`）。
    pub about: String,
}

impl Profile {
    /// 照模板 `template` 拼成一块：两项去掉首尾空白、截到上限，称呼里的换行换成空格；空的那一项所在的行（写着 `{name}`、
    /// `{about}` 的那一行）不写；两项都空的没有。原文不转义：是本人写的，同人设（群聊不带，`venues.md`）。
    pub fn block(&self, template: &str) -> Option<String> {
        let name = clip(self.name.trim(), NAME_MAX).replace(['\n', '\r'], " ");
        let about = clip(self.about.trim(), ABOUT_MAX);
        if name.is_empty() && about.is_empty() {
            return None;
        }
        let lines: Vec<String> = template
            .lines()
            .filter(|line| !(line.contains("{name}") && name.is_empty()))
            .filter(|line| !(line.contains("{about}") && about.is_empty()))
            .map(|line| line.replace("{name}", &name).replace("{about}", about))
            .collect();
        Some(lines.join("\n"))
    }
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
