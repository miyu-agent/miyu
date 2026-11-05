//! `cargo xtask packaging <标签> --sums <SHA256SUMS> [--out <目录>]`（施工 RL-5，设计 `60-系统源分包.md` 第二节、第五节）：
//! 照一次发布的 `SHA256SUMS` 填系统源的模板，填出能发的 AUR `PKGBUILD` 和 Homebrew formula。
//!
//! - 模板在 `packaging/aur/<包>/PKGBUILD.in`、`packaging/homebrew/<包>.rb.in`，`miyu-agent-git` 的 `PKGBUILD` 不是模板、照原样拷；
//! - 占位：`@VERSION@` 核心的版本（发版标签去掉 `v`，下载地址照它）、`@PKG_VERSION@` 这个包自己的版本（终端、网页照压缩包名里
//!   写的，同它们自己的清单）、`@PKGVER@` 同 `@PKG_VERSION@` 但去掉 `-`（pacman 的 pkgver 不许有 `-`，`0.0.0-rc.5` 写成
//!   `0.0.0rc.5`，照样排在 `0.0.0` 前面）、`@SHA256_<系统>@` 这个包那一种系统的压缩包的哈希、`@SHA256_SUMS@` 这份 `SHA256SUMS` 自己的
//!   哈希（Homebrew 的合集 formula 拿它当下载）；
//! - 填完还有 `@…@` 的、`SHA256SUMS` 里缺哪一份的、终端网页三种系统写的版本不一样的，都报错、不写。
//!
//! `SHA256SUMS` 照本地的一份读：发版那天先 `gh release download` 下来、`miyu-index verify-sums` 核过签名再交给这里。放在
//! `--out`，不写的放 cargo 的 target 目录下的 `packaging/<标签>/`。`.SRCINFO` 要 `makepkg --printsrcinfo` 出，在 Arch 上做。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sha2::{Digest, Sha256};

/// 出系统源的几种系统（Windows 没有，设计 60 第五节第 3 条），和占位里的写法。
const TARGETS: [(&str, &str); 3] = [
    ("linux-x86_64", "LINUX_X86_64"),
    ("linux-aarch64", "LINUX_AARCH64"),
    ("macos-aarch64", "MACOS_AARCH64"),
];

/// 填一份：模板（相对 `packaging/`）、填出来放哪（相对 `--out`）、照哪个分包（`<编号>-<版本>-<系统>.tar.gz`）的版本和哈希填。
const FILES: [(&str, &str, &str); 8] = [
    (
        "aur/miyu-core/PKGBUILD.in",
        "aur/miyu-core/PKGBUILD",
        "miyu-core",
    ),
    (
        "aur/miyu-tui/PKGBUILD.in",
        "aur/miyu-tui/PKGBUILD",
        "miyu-tui",
    ),
    (
        "aur/miyu-webui/PKGBUILD.in",
        "aur/miyu-webui/PKGBUILD",
        "miyu-webui",
    ),
    (
        "aur/miyu-agent/PKGBUILD.in",
        "aur/miyu-agent/PKGBUILD",
        "miyu-core",
    ),
    (
        "homebrew/miyu-core.rb.in",
        "homebrew/Formula/miyu-core.rb",
        "miyu-core",
    ),
    (
        "homebrew/miyu-tui.rb.in",
        "homebrew/Formula/miyu-tui.rb",
        "miyu-tui",
    ),
    (
        "homebrew/miyu-webui.rb.in",
        "homebrew/Formula/miyu-webui.rb",
        "miyu-webui",
    ),
    (
        "homebrew/miyu-agent.rb.in",
        "homebrew/Formula/miyu-agent.rb",
        "miyu-core",
    ),
];

/// 照原样拷的（不是模板）。
const COPIED: [(&str, &str); 1] = [("aur/miyu-agent-git/PKGBUILD", "aur/miyu-agent-git/PKGBUILD")];

/// `cargo xtask packaging <标签> --sums <SHA256SUMS> [--out <目录>]`。
pub fn run(root: &Path, target_dir: Option<PathBuf>, args: &[String]) -> ExitCode {
    match packaging(root, target_dir, args) {
        Ok(written) => {
            for path in written {
                println!("写好了 {}", path.display());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("packaging：{error}");
            ExitCode::FAILURE
        }
    }
}

fn packaging(
    root: &Path,
    target_dir: Option<PathBuf>,
    args: &[String],
) -> Result<Vec<PathBuf>, String> {
    let (tag, sums, out) = options(args)?;
    let version = tag
        .strip_prefix('v')
        .filter(|rest| !rest.is_empty())
        .ok_or_else(|| format!("标签要是 v<版本>，写的是 {tag}"))?;
    let bytes = fs::read(&sums).map_err(|error| format!("{}：{error}", sums.display()))?;
    let release = Release::read(version, &bytes)?;
    let out = match out {
        Some(out) => out,
        None => target_dir
            .ok_or("没写 --out，也找不到 cargo 的 target 目录")?
            .join("packaging")
            .join(&tag),
    };
    // 先全填好，哪一份不成都不写。
    let mut ready = Vec::new();
    for (template, to, part) in FILES {
        let text = read(&root.join("packaging").join(template))?;
        let filled =
            fill(&text, &release.values(part)?).map_err(|error| format!("{template}：{error}"))?;
        ready.push((out.join(to), filled));
    }
    for (from, to) in COPIED {
        ready.push((out.join(to), read(&root.join("packaging").join(from))?));
    }
    ready.iter().map(|(path, text)| write(path, text)).collect()
}

fn options(args: &[String]) -> Result<(String, PathBuf, Option<PathBuf>), String> {
    let usage = "用法：cargo xtask packaging <标签> --sums <SHA256SUMS> [--out <目录>]";
    let mut tag = None;
    let mut sums = None;
    let mut out = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--sums" => sums = Some(PathBuf::from(rest.next().ok_or(usage)?)),
            "--out" => out = Some(PathBuf::from(rest.next().ok_or(usage)?)),
            other if tag.is_none() && !other.starts_with("--") => tag = Some(other.to_string()),
            other => return Err(format!("不认识的参数 {other}；{usage}")),
        }
    }
    Ok((tag.ok_or(usage)?, sums.ok_or(usage)?, out))
}

/// 一次发布：核心的版本、`SHA256SUMS` 里每个文件的哈希、`SHA256SUMS` 自己的哈希。
pub(crate) struct Release {
    version: String,
    sums: BTreeMap<String, String>,
    own: String,
}

impl Release {
    /// 读 `SHA256SUMS` 的字节（`<哈希>  <文件名>` 一行一个，`sha256sum` 的写法；二进制模式的 `*` 去掉）。
    pub(crate) fn read(version: &str, bytes: &[u8]) -> Result<Release, String> {
        let text = std::str::from_utf8(bytes).map_err(|_| "SHA256SUMS 不是 UTF-8".to_string())?;
        let mut sums = BTreeMap::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let (hash, name) = line
                .split_once(char::is_whitespace)
                .ok_or_else(|| format!("SHA256SUMS 这一行读不懂：{line}"))?;
            let name = name.trim_start().trim_start_matches('*');
            if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(format!("SHA256SUMS 这一行的哈希不对：{line}"));
            }
            sums.insert(name.to_string(), hash.to_ascii_lowercase());
        }
        Ok(Release {
            version: version.to_string(),
            sums,
            own: hex(&Sha256::digest(bytes)),
        })
    }

    /// 分包 `part` 自己的版本：核心照发版的版本；终端、网页照压缩包名，三种系统要写的一样。
    fn part_version(&self, part: &str) -> Result<String, String> {
        if part == "miyu-core" {
            return Ok(self.version.clone());
        }
        let mut found: Vec<String> = Vec::new();
        for (target, _) in TARGETS {
            let suffix = format!("-{target}.tar.gz");
            let version = self
                .sums
                .keys()
                .find_map(|name| {
                    let rest = name
                        .strip_prefix(&format!("{part}-"))?
                        .strip_suffix(&suffix)?;
                    (!rest.is_empty()).then(|| rest.to_string())
                })
                .ok_or_else(|| format!("SHA256SUMS 里没有 {part}-<版本>{suffix}"))?;
            found.push(version);
        }
        found.dedup();
        match found.as_slice() {
            [one] => Ok(one.clone()),
            _ => Err(format!("{part} 三种系统写的版本不一样：{found:?}")),
        }
    }

    /// 照分包 `part` 填的那几个值。
    fn values(&self, part: &str) -> Result<Vec<(String, String)>, String> {
        let version = self.part_version(part)?;
        let mut values = vec![
            ("VERSION".to_string(), self.version.clone()),
            ("PKG_VERSION".to_string(), version.clone()),
            ("PKGVER".to_string(), version.replace('-', "")),
            ("SHA256_SUMS".to_string(), self.own.clone()),
        ];
        for (target, key) in TARGETS {
            let name = format!("{part}-{version}-{target}.tar.gz");
            let hash = self
                .sums
                .get(&name)
                .ok_or_else(|| format!("SHA256SUMS 里没有 {name}"))?;
            values.push((format!("SHA256_{key}"), hash.clone()));
        }
        Ok(values)
    }
}

/// 把模板里的 `@<名字>@` 换成值；换完还有 `@` 加大写字母的占位的报出来。
pub(crate) fn fill(template: &str, values: &[(String, String)]) -> Result<String, String> {
    let mut text = template.to_string();
    for (key, value) in values {
        text = text.replace(&format!("@{key}@"), value);
    }
    // 逐个 `@` 往后看到下一个 `@`：中间是一个以上的大写字母、数字、下划线的，是没填的占位（邮箱那种不算）。
    let mut left = Vec::new();
    let mut rest = text.as_str();
    while let Some(start) = rest.find('@') {
        let after = &rest[start + 1..];
        match after.find('@') {
            Some(end)
                if end > 0
                    && after[..end].bytes().all(|byte| {
                        byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_'
                    }) =>
            {
                left.push(after[..end].to_string());
                rest = &after[end + 1..];
            }
            _ => rest = after,
        }
    }
    match left.as_slice() {
        [] => Ok(text),
        _ => Err(format!("还有没填的占位：{left:?}")),
    }
}

/// 小写十六进制。
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("{}：{error}", path.display()))
}

fn write(path: &Path, text: &str) -> Result<PathBuf, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}：{error}", parent.display()))?;
    }
    fs::write(path, text).map_err(|error| format!("{}：{error}", path.display()))?;
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests;
