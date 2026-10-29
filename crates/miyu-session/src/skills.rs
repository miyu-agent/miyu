//! 她用得上的技能（施工 X-10 上，`docs/blueprint/skills.md`「技能」第 2、3 条）：找、system 里列出来的那一块、权限策略单独
//! 放开只读的文件夹。
//!
//! 造会话时照这一刻装了的软件包和家目录里的找（`miyu_store::skills`），拼好的一块钉进快照，会话中途不改（列表变了的走
//! 回合尾巴，施工 X-11）；放开只读的文件夹造会话、载入时各找一次。碰磁盘的在阻塞线程里做。

use std::collections::BTreeMap;
use std::path::PathBuf;

use miyu_kernel::template::Template;
use miyu_policy::ToolEntry;
use miyu_policy::preset::PresetFile;
use miyu_policy::skill::feature_id;
use miyu_store::packages::Found;
use miyu_store::resources::{ResourceRoot, SourceError};
use miyu_store::root::DataRoot;
use miyu_store::skills::{self, External, Skill};
use miyu_tool::load::say;

use crate::TARGET;
use crate::path_text::path_text;

/// 读技能用的那件工具（基本系统的 `read`）：工具面上没有它的，列出来她也读不了，不列。
const READ: &str = "read";

/// 找属主 `owner` 用得上的技能：装了的软件包 `installed` 里的，加数据根 `root` 里他自己放的，加配置开着时别的目录 `external`
/// 里的（施工 PT-2 外）。跳过的一个记一行，只写哪一种（同名被盖住的、链接指到外面的、别的），路径、原因里有人起的名字，不写
/// （施工 LG-1 上）；别的目录读不了的静默跳过，记一行 debug。
pub(crate) fn find(installed: &[Found], root: &DataRoot, external: &[External]) -> Vec<Skill> {
    for one in external.iter().filter(|one| !one.dir.is_dir()) {
        tracing::debug!(target: TARGET, chars = one.text.chars().count(), "external skills folder not read");
    }
    let (found, skipped) = skills::find(installed, root, external);
    for one in &skipped {
        let kind = match (&one.by, &one.outside) {
            (Some(_), _) => "covered",
            (None, Some(_)) => "outside",
            (None, None) => "other",
        };
        tracing::warn!(target: TARGET, kind, "skill skipped");
    }
    found
}

/// 会话的预设 `preset` 开着的技能（施工 X-11 上，`skills.md`「定了的」）：每个技能算一个功能 `skill-<名字>`，照预设的
/// `[features]`、老写法 `[software]`（包带的认包的编号）、`unlisted` 判；没有预设的全开。
pub(crate) fn opened(found: Vec<Skill>, preset: Option<&PresetFile>) -> Vec<Skill> {
    let Some(preset) = preset else {
        return found;
    };
    found
        .into_iter()
        .filter(|skill| {
            preset.opens_in(
                &feature_id(&skill.head.name),
                skill.package.as_deref().unwrap_or_default(),
            )
        })
        .collect()
}

/// 她的 system 里列出来的那一块（`skills.md`「技能」第 2 条）：头一行，一个技能一行（名字、说明、`SKILL.md` 的路径），字从
/// 资源目录的 `core/skills/` 读，三格照模板转义。工具面 `face` 上没有 `read` 的、一个技能都没有的没有这一块；只许人点名用的（施工 PT-2 下）不列。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对：安装坏了，照随核心附带的字读不出来报。
pub(crate) fn listing(
    resources: &ResourceRoot,
    face: &[ToolEntry],
    skills: &[Skill],
) -> Result<Option<String>, SourceError> {
    // 只许人点名用的不列（施工 PT-2 下，`disable-model-invocation`）：她自己不该挑它。
    let skills: Vec<&Skill> = skills
        .iter()
        .filter(|skill| skill.head.model_invocable)
        .collect();
    if skills.is_empty() || !face.iter().any(|tool| tool.name == READ) {
        return Ok(None);
    }
    let texts = resources.skill_texts()?;
    let line = template(&texts.line).map_err(|why| SourceError::Read {
        path: resources
            .path()
            .join("core")
            .join("skills")
            .join("line.txt"),
        error: std::io::Error::new(std::io::ErrorKind::InvalidData, why),
    })?;
    let mut lines = vec![texts.open.trim_end().to_string()];
    for skill in skills {
        let path = path_text(&skill.file);
        lines.push(
            say(
                &line,
                &[
                    ("name", &skill.head.name),
                    ("description", &skill.head.description),
                    ("path", &path),
                ],
            )
            .trim_end()
            .to_string(),
        );
    }
    Ok(Some(lines.join("\n")))
}

/// 一行的模板：读得成，三格换得进去。
fn template(source: &str) -> Result<Template, String> {
    let template = Template::parse(source).map_err(|error| error.why)?;
    let trial = BTreeMap::from([("name", ""), ("description", ""), ("path", "")]);
    template.render(&trial).map_err(|error| error.why)?;
    Ok(template)
}

/// 技能放开只读的文件夹：每个技能一个，换成真实的位置（换不成的照原样）：权限策略、沙盒、认用过的技能都照真实的位置比
/// （施工 X-10 下）。碰磁盘，在阻塞线程里调。
pub(crate) fn shelves(skills: &[Skill]) -> Vec<PathBuf> {
    skills
        .iter()
        .map(|skill| std::fs::canonicalize(&skill.dir).unwrap_or_else(|_| skill.dir.clone()))
        .collect()
}

/// 开着的技能给人看的名字（施工 9-8 五补）：技能名到它 `SKILL.md` 的 `display_name`（照语言）。没写显示名的不在里面。
pub(crate) type Names = BTreeMap<String, BTreeMap<String, String>>;

/// 这几个开着的技能给人看的名字：读它们的 `SKILL.md` 时记进效果，历史照它写，卸了包、改了名字照旧。
pub(crate) fn names(skills: &[Skill]) -> Names {
    skills
        .iter()
        .filter(|skill| !skill.head.display_name.is_empty())
        .map(|skill| (skill.head.name.clone(), skill.head.display_name.clone()))
        .collect()
}

mod watch;

pub(crate) use watch::{Watch, tail};

#[cfg(test)]
mod tests;
