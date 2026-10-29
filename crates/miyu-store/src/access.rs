//! 访问密码 `access.json`（施工 SU-6 上：只给一个人用，原来的账号表 `accounts.json` 只剩主人那一行的密码，`docs/blueprint/
//! web-module.md`「怎么走」第一条第 1 条，照 Linux 的 `/etc/shadow`）：argon2id 的整串（带参数和盐）、改的时刻。只在要从局域网
//! 连时才设；没设的是空的。
//!
//! 密码哈希照设计 06 U4：argon2id，参数 `m=19456`（KiB）、`t=2`、`p=1`（OWASP 的推荐值之一，「起草时定的」第 43 条），
//! 盐是 16 个系统给的随机字节。参数写进整串里，以后调了旧的照样验得过。算一次几十毫秒、占 19 MiB，调用的一方放进阻塞
//! 线程。

use std::path::Path;

use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};

use miyu_kernel::time::Timestamp;

use crate::config_file::WriteError;
use crate::private_json;
pub use crate::private_json::BadFile;

/// 文件名：在数据根下。
pub const FILE: &str = "access.json";

/// 现在的写法。
const VERSION: u32 = 1;

/// 读好的一份。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Access {
    /// 写法的版本：现在是 1。
    pub version: u32,
    /// argon2id 的整串（PHC 写法）；没设的是空的。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub password: String,
    /// 什么时候设的；没设的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed: Option<Timestamp>,
}

impl Default for Access {
    fn default() -> Access {
        Access {
            version: VERSION,
            password: String::new(),
            changed: None,
        }
    }
}

/// 哈希不印。
impl std::fmt::Debug for Access {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Access")
            .field("set", &!self.password.is_empty())
            .field("changed", &self.changed)
            .finish_non_exhaustive()
    }
}

/// 读到的一份和它的版本（写回时交给 [`write()`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// 读好的。没有这个文件的是空的。
    pub access: Access,
    /// 整份字节的 SHA-256；没有这个文件的是空的。
    pub version: Option<String>,
}

/// 读 `path`。没有这个文件的是空的。
///
/// # Errors
///
/// 读不了、太大、不是 UTF-8；不是这个形状、版本不是 1。
pub fn read(path: &Path) -> Result<Read, BadFile> {
    let (access, version): (Access, _) = private_json::read(path)?;
    if access.version != VERSION {
        return Err(BadFile::Shape(format!(
            "unknown version {}",
            access.version
        )));
    }
    Ok(Read { access, version })
}

/// 把 `access` 整份写进 `path`，Unix 上 0600。`read` 是上一次读到的版本（那时还没有的是空的）。
///
/// # Errors
///
/// 这一瞬间有人手改了（[`WriteError::Changed`]）；写不进。都是什么都没变。
pub fn write(path: &Path, access: &Access, read: Option<&str>) -> Result<(), WriteError> {
    private_json::write(path, access, read)
}

/// 算 `password` 的 argon2id 整串。
///
/// # Errors
///
/// 拿不到随机的盐（系统给不出随机字节）；算不出（参数不合法，不会发生）。
pub fn hash(password: &str) -> Result<String, String> {
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|error| error.to_string())?;
    let hashed = hasher()?
        .hash_password_with_salt(password.as_bytes(), &salt)
        .map_err(|error| error.to_string())?;
    Ok(hashed.to_string())
}

/// `password` 和整串 `phc` 对不对得上。读不懂的整串当对不上。
pub fn verify(phc: &str, password: &str) -> bool {
    hasher().is_ok_and(|hasher| hasher.verify_password(password.as_bytes(), phc).is_ok())
}

/// 照图纸的参数造的 argon2id。
fn hasher() -> Result<Argon2<'static>, String> {
    let params = Params::new(19_456, 2, 1, None).map_err(|error| error.to_string())?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

#[cfg(test)]
mod tests;
