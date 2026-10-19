//! 项目 `home/<账号>/projects.json`（施工 H-1，设计 `38-项目.md` 第一节）：一个账号的几个项目，照造的先后。哪个会话在哪个项目
//! 记在会话自己的日志里，这里不另记名单。读、写照 `logins.json`（只给自己看的 JSON 小文件：0600、照读到的版本写）。

use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize};

use miyu_kernel::time::Timestamp;

use crate::config_file::WriteError;
use crate::private_json;
pub use crate::private_json::BadFile;

/// 文件名：在账号的家目录里。
pub const FILE: &str = "projects.json";

/// 现在的写法。
const VERSION: u32 = 1;

/// 名称最多几个字。
pub const NAME_MOST: usize = 40;

/// 说明最多几个字。
pub const DESCRIPTION_MOST: usize = 80;

/// 附加提示词最多几个字。
pub const PROMPT_MOST: usize = 8000;

/// 读好的一份。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Projects {
    /// 写法的版本：现在是 1。
    pub version: u32,
    /// 一个项目一项，照造的先后。
    pub projects: Vec<Project>,
}

impl Default for Projects {
    fn default() -> Projects {
        Projects {
            version: VERSION,
            projects: Vec::new(),
        }
    }
}

/// 一个项目。没写的格不写进文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// 编号：UUIDv7 的写法，核心起。
    pub project: String,
    /// 名称。
    pub name: String,
    /// 说明。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 颜色：`#rrggbb`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 新建会话的工作目录，人看到的写法。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// 新建会话的人格：没有这一格是照配置的默认，`null` 是无人格，字是人格编号。
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub persona: Option<Option<String>>,
    /// 新建会话的预设。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// 附加提示词。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// 能和项目外的会话互相留言。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub open: bool,
    /// 造的时刻。
    pub created: Timestamp,
}

/// 有这一格的照它读（`null` 是 `Some(None)`）；没有这一格的由 `default` 给 `None`。
fn present<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}

/// 哪一格写得不对（给 `data.field`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrong {
    /// 名称空的、超过 [`NAME_MOST`] 个字。
    Name,
    /// 说明超过 [`DESCRIPTION_MOST`] 个字。
    Description,
    /// 颜色不是 `#rrggbb`。
    Color,
    /// 工作目录空的。
    Workspace,
    /// 附加提示词超过 [`PROMPT_MOST`] 个字。
    Prompt,
}

impl Wrong {
    /// 格的名字。
    pub fn field(self) -> &'static str {
        match self {
            Wrong::Name => "name",
            Wrong::Description => "description",
            Wrong::Color => "color",
            Wrong::Workspace => "workspace",
            Wrong::Prompt => "prompt",
        }
    }
}

impl Project {
    /// 查各格的写法：名称去掉首尾空白以后 1 到 [`NAME_MOST`] 个字，说明、附加提示词不超过上限，颜色是 `#rrggbb`，工作目录不是
    /// 空的。人格、预设在不在由用的一方查。
    ///
    /// # Errors
    ///
    /// 头一处写得不对的格。
    pub fn check(&self) -> Result<(), Wrong> {
        let name = self.name.trim();
        if name.is_empty() || name.chars().count() > NAME_MOST || name != self.name {
            return Err(Wrong::Name);
        }
        if too_long(self.description.as_deref(), DESCRIPTION_MOST) {
            return Err(Wrong::Description);
        }
        if self.color.as_deref().is_some_and(|color| !color_ok(color)) {
            return Err(Wrong::Color);
        }
        if self
            .workspace
            .as_deref()
            .is_some_and(|dir| dir.trim().is_empty())
        {
            return Err(Wrong::Workspace);
        }
        if too_long(self.prompt.as_deref(), PROMPT_MOST) {
            return Err(Wrong::Prompt);
        }
        Ok(())
    }
}

/// 超过 `most` 个字。
fn too_long(text: Option<&str>, most: usize) -> bool {
    text.is_some_and(|text| text.chars().count() > most)
}

/// `#` 加六位十六进制。
fn color_ok(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

impl Projects {
    /// 编号是 `id` 的那一个。
    pub fn find(&self, id: &str) -> Option<&Project> {
        self.projects.iter().find(|project| project.project == id)
    }

    /// 同 [`find`](Projects::find)，能改。
    pub fn find_mut(&mut self, id: &str) -> Option<&mut Project> {
        self.projects
            .iter_mut()
            .find(|project| project.project == id)
    }

    /// 拿掉编号是 `id` 的那一个：有的交回它。
    pub fn remove(&mut self, id: &str) -> Option<Project> {
        let at = self
            .projects
            .iter()
            .position(|project| project.project == id)?;
        Some(self.projects.remove(at))
    }
}

/// 读到的一份和它的版本（写回时交给 [`write()`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// 读好的。没有这个文件的是空的。
    pub projects: Projects,
    /// 整份字节的 SHA-256；没有这个文件的是空的。
    pub version: Option<String>,
}

/// 读 `path`。没有这个文件的是空的。
///
/// # Errors
///
/// 读不了、太大、不是 UTF-8；不是这个形状、版本不是 1。
pub fn read(path: &Path) -> Result<Read, BadFile> {
    let (projects, version): (Projects, _) = private_json::read(path)?;
    if projects.version != VERSION {
        return Err(BadFile::Shape(format!(
            "unknown version {}",
            projects.version
        )));
    }
    Ok(Read { projects, version })
}

/// 把 `projects` 整份写进 `path`，Unix 上 0600。`read` 是上一次读到的版本（那时还没有的是空的）。
///
/// # Errors
///
/// 这一瞬间有人手改了（[`WriteError::Changed`]）；写不进。都是什么都没变。
pub fn write(path: &Path, projects: &Projects, read: Option<&str>) -> Result<(), WriteError> {
    private_json::write(path, projects, read)
}

#[cfg(test)]
mod tests;
