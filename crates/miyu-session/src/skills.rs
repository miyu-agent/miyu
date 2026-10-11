//! 她用得上的技能（施工 X-10 上，`docs/blueprint/skills.md`「技能」第 2、3 条）：找、system 里列出来的那一块、权限策略单独
//! 放开只读的文件夹。
//!
//! 造会话时照这一刻装了的软件包和家目录里的找（`miyu_store::skills`），拼好的一块钉进快照，会话中途不改（列表变了的走
//! 回合尾巴，施工 X-11）；放开只读的文件夹造会话、载入时各找一次。碰磁盘的在阻塞线程里做。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use miyu_kernel::id::AccountId;
use miyu_kernel::template::Template;
use miyu_policy::ToolEntry;
use miyu_store::packages::Found;
use miyu_store::resources::{ResourceRoot, SourceError};
use miyu_store::root::DataRoot;
use miyu_store::skills::{self, Skill};
use miyu_tool::load::say;

use crate::TARGET;

/// 读技能用的那件工具（基本系统的 `read`）：工具面上没有它的，列出来她也读不了，不列。
const READ: &str = "read";

/// 找属主 `owner` 用得上的技能：装了的软件包 `installed` 里的，加数据根 `root` 里他自己放的。跳过的一个记一行。
pub(crate) fn find(installed: &[Found], root: &DataRoot, owner: &AccountId) -> Vec<Skill> {
    let (found, skipped) = skills::find(installed, root, owner);
    for one in &skipped {
        tracing::warn!(
            target: TARGET,
            path = %one.path.display(),
            why = one.why.as_str(),
            "skill skipped"
        );
    }
    found
}

/// 她的 system 里列出来的那一块（`skills.md`「技能」第 2 条）：头一行，一个技能一行（名字、说明、`SKILL.md` 的路径），字从
/// 资源目录的 `core/skills/` 读，三格照模板转义。工具面 `face` 上没有 `read` 的、一个技能都没有的没有这一块。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对：安装坏了，照随核心附带的字读不出来报。
pub(crate) fn listing(
    resources: &ResourceRoot,
    face: &[ToolEntry],
    skills: &[Skill],
) -> Result<Option<String>, SourceError> {
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
        let path = shown(&skill.file);
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

/// 技能放开只读的文件夹：每个技能一个。
pub(crate) fn shelves(skills: &[Skill]) -> Vec<PathBuf> {
    skills.iter().map(|skill| skill.dir.clone()).collect()
}

/// 路径写给她看的样子：Windows 上反斜杠换成 `/`（转义会把反斜杠写成两个，她照抄就对不上；`/` 在 Windows 上一样认）。
fn shown(path: &Path) -> String {
    let text = path.to_string_lossy();
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text.into_owned()
    }
}

#[cfg(test)]
mod tests;
