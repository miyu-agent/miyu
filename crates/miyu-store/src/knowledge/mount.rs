//! 库的文档在哪（施工 K-4，`docs/blueprint/knowledge.md`「对外的样子」挂进来的那一段、「怎么走」第三条第 6 款）：自己的库在
//! 它自己的目录里；`kb.toml` 写了 `mount` 的是挂进来的，文档在那个目录里，只读（19 N7）。挂的路径写绝对路径，或者 `~/` 开头
//! 照核心的家目录（同 `miyu_fs::tilde` 那一套）；用不了的交回为什么，调的一方照它跳过、报给人看，索引不动。

use std::path::{Path, PathBuf};

use miyu_kernel::id::AccountId;

use super::catch_up::about;
use super::{Knowledge, KnowledgeError};

/// 一个库的文档在哪。
pub(super) enum Documents {
    /// 库自己的目录。
    Own(PathBuf),
    /// 挂进来的目录；用不了的是为什么（英文的一句）。
    Mounted(Result<PathBuf, String>),
}

impl Documents {
    /// 照这个目录补、读：挂的目录用不了的是 [`KnowledgeError::Unavailable`]。
    pub(super) fn usable(self) -> Result<PathBuf, KnowledgeError> {
        match self {
            Documents::Own(dir) | Documents::Mounted(Ok(dir)) => Ok(dir),
            Documents::Mounted(Err(why)) => Err(KnowledgeError::Unavailable(why)),
        }
    }
}

impl Knowledge {
    /// 库 `id` 的文档在哪。`kb.toml` 读不出来的照没写（同 [`Knowledge::list`] 带的 `error`）。
    pub(super) fn documents(
        &self,
        account: &AccountId,
        id: &str,
    ) -> Result<Documents, KnowledgeError> {
        let dir = self.dir(account, id)?;
        let (about, _) = about(&dir);
        Ok(match about.mount {
            None => Documents::Own(dir),
            Some(mount) => Documents::Mounted(self.mounted(&mount)),
        })
    }

    /// 挂的路径 `mount` 换成一个读得了的目录。
    pub(super) fn mounted(&self, mount: &str) -> Result<PathBuf, String> {
        let path = match miyu_fs::tilde(mount) {
            // 一段段接：Windows 上 `/`、`\` 都写得出来。
            Some(rest) => rest
                .split(['/', '\\'])
                .filter(|part| !part.is_empty())
                .fold(self.home.clone().ok_or(NO_HOME)?, |path, part| {
                    path.join(part)
                }),
            None if Path::new(mount).is_absolute() => PathBuf::from(mount),
            None => return Err(format!("mount {mount} is not an absolute path or ~/")),
        };
        match std::fs::read_dir(&path) {
            Ok(_) => Ok(path),
            Err(error) => Err(format!("mounted folder {}: {error}", path.display())),
        }
    }
}

const NO_HOME: &str = "mount starts with ~ but the home directory is not known";
