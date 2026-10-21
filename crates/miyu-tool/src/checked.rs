//! 工具执行时换真实的位置（施工 5-18，`11-权限与沙盒.md` 第七节）：照旧换，再核对是权限策略判过的那一个。
//!
//! 权限策略判的时候换了一次，工具跑的时候又换一次。两次之间路径上的链接被换掉（沙盒里的命令就换得了工作区里的链接），
//! 第二次换出来的就不是判过的那一个：可能落进数据根、私密目录，或者工作区一级不问人写到工作区外。所以工具换出来的
//! 要在判过的里面（[`Call::checked`]），不在的不碰；换出来以后到打开之间，照旧由逐层不跟链接地打开挡（`miyu_fs`）。

use std::fmt;
use std::path::{Path, PathBuf};

use miyu_fs::{ResolveError, resolve, resolve_itself};

use crate::Call;

/// 换不成真实的位置，或者换出来的不是判过的。
#[derive(Debug)]
pub enum Unchecked {
    /// 换不成（[`miyu_fs::resolve()`] 的那几种）。
    Resolve(ResolveError),
    /// 换出来的不在权限策略判过的里面：判完以后路径上的链接被换过。
    Moved,
}

impl fmt::Display for Unchecked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unchecked::Resolve(error) => write!(f, "{error}"),
            Unchecked::Moved => f.write_str("the path changed after the permission check"),
        }
    }
}

impl std::error::Error for Unchecked {}

impl Call {
    /// 把她给的路径 `path` 换成真实的位置（[`miyu_fs::resolve()`]：相对的照工作目录接上，`~` 开头的照家目录），再核对是
    /// 权限策略判过的那一个。
    ///
    /// # Errors
    ///
    /// 换不成；换出来的不在 [`Call::checked`] 里。
    pub fn real(&self, path: &str) -> Result<PathBuf, Unchecked> {
        let real = resolve(Path::new(&self.cwd), self.home.as_deref(), path)
            .map_err(Unchecked::Resolve)?;
        self.verified(real)
    }

    /// 同 [`Call::real`]，最后一段不跟链接（[`miyu_fs::resolve_itself`]，`trash` 删的是链接本身）。没有名字可碰的（`.`、
    /// `~` 这些）交回空的，工具不碰，用不着核对。
    ///
    /// # Errors
    ///
    /// 换不成；换出来的不在 [`Call::checked`] 里。
    pub fn real_itself(&self, path: &str) -> Result<Option<PathBuf>, Unchecked> {
        match resolve_itself(Path::new(&self.cwd), self.home.as_deref(), path) {
            Ok(Some(real)) => self.verified(real).map(Some),
            Ok(None) => Ok(None),
            Err(error) => Err(Unchecked::Resolve(error)),
        }
    }

    /// 判过的里面有 `real` 才交回；没带判过的（测试里直接造的调用）照原样交回。
    fn verified(&self, real: PathBuf) -> Result<PathBuf, Unchecked> {
        match &self.checked {
            Some(checked) if !checked.contains(&real) => Err(Unchecked::Moved),
            _ => Ok(real),
        }
    }
}

#[cfg(test)]
mod tests;
