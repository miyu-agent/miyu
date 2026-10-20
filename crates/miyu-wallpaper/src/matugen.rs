//! matugen 生成的那份颜色（蓝图 `web.md`「跟着人格的外观」第 5 条，2026-10-11 项目主人定：有它就先照它）：用 matugen 换壁纸的，照仓库给的
//! 模板（`web/matugen/miyu-web.json`）生成到固定的地方，这里只管找到它、给记号、读出来；里面写的什么由页面认（`src/lib/md3/`）。
//!
//! 固定的地方：Linux、BSD `$XDG_CACHE_HOME/miyu/matugen.json`（没设的 `~/.cache/miyu/`），macOS `~/Library/Caches/miyu/matugen.json`，
//! Windows `%LOCALAPPDATA%\miyu\matugen.json`。

use std::io;
use std::path::PathBuf;

/// 这份颜色最大多大：模板生成的只有几 KB，再大的不是它。
pub const MAX_BYTES: u64 = 256 * 1024;

/// 找到的那份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matugen {
    /// 文件的绝对路径。
    pub path: PathBuf,
}

impl Matugen {
    /// 换没换的记号（同壁纸的写法）。
    ///
    /// # Errors
    ///
    /// 读不了文件的信息。
    pub fn signature(&self) -> io::Result<String> {
        crate::signature_of(&self.path)
    }

    /// 读出来（JSON 的字节，不在这里认）。
    ///
    /// # Errors
    ///
    /// 读不了、超过 [`MAX_BYTES`]。
    pub fn read(&self) -> io::Result<Vec<u8>> {
        crate::read_capped(&self.path, MAX_BYTES)
    }
}

/// matugen 生成到哪（这台电脑上的固定地方）；连家目录都找不到的是 `None`。
pub fn output_path() -> Option<PathBuf> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let dir = if cfg!(windows) {
        env("LOCALAPPDATA")?
    } else if cfg!(target_os = "macos") {
        env("HOME")?.join("Library/Caches")
    } else {
        env("XDG_CACHE_HOME").or_else(|| env("HOME").map(|h| h.join(".cache")))?
    };
    Some(dir.join("miyu").join("matugen.json"))
}

/// 有没有 matugen 生成的那一份。
#[must_use]
pub fn find() -> Option<Matugen> {
    output_path()
        .filter(|p| p.is_file())
        .map(|path| Matugen { path })
}
