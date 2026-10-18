//! 包能装在哪几种系统上（施工 G-1，设计 `37-软件源.md` 第二节第 3 条）：清单 `[package] targets` 写的。没写的是哪种都能装；
//! 带了本机程序的包写它能跑的几种，一种一个包文件。「平台」在清单里说的是通讯平台（`[connection] platform`），这里叫
//! 「系统」。

use toml_edit::TableLike;

use super::reader::Reader;
use super::{Code, Problem};

/// 一种系统：操作系统加处理器。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Target {
    /// Linux，x86-64。
    LinuxX86_64,
    /// Linux，ARM64。
    LinuxAarch64,
    /// macOS，Apple 芯片。
    MacosAarch64,
    /// Windows，x86-64。
    WindowsX86_64,
}

impl Target {
    /// 全部，照清单里的先后。
    pub const ALL: [Target; 4] = [
        Target::LinuxX86_64,
        Target::LinuxAarch64,
        Target::MacosAarch64,
        Target::WindowsX86_64,
    ];

    /// 清单、包文件名、索引里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Target::LinuxX86_64 => "linux-x86_64",
            Target::LinuxAarch64 => "linux-aarch64",
            Target::MacosAarch64 => "macos-aarch64",
            Target::WindowsX86_64 => "windows-x86_64",
        }
    }

    /// 照写法认；不认识的没有。
    pub fn parse(text: &str) -> Option<Target> {
        Target::ALL
            .into_iter()
            .find(|target| target.as_str() == text)
    }

    /// 这个程序编出来跑在哪一种上；不在这四种里的没有。编译时定的，不读环境。
    pub fn here() -> Option<Target> {
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some(Target::LinuxX86_64)
        } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            Some(Target::LinuxAarch64)
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some(Target::MacosAarch64)
        } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            Some(Target::WindowsX86_64)
        } else {
            None
        }
    }
}

impl Reader<'_> {
    /// `[package] targets`：字的列表，每个都是认识的系统、不重复；照 [`Target::ALL`] 的先后排好。没写的是空的。
    pub(super) fn targets(&self, table: &dyn TableLike) -> Result<Vec<Target>, Problem> {
        let mut targets = Vec::new();
        for name in &self.texts(table, "package", "targets")? {
            match Target::parse(name) {
                Some(target) if !targets.contains(&target) => targets.push(target),
                found => {
                    let why = match found {
                        Some(_) => "is listed twice",
                        None => {
                            "is not a known target (linux-x86_64, linux-aarch64, macos-aarch64, windows-x86_64)"
                        }
                    };
                    return Err(self.problem(
                        table.get("targets"),
                        Code::BadTargets,
                        name,
                        format!("package.targets: \"{name}\" {why}"),
                    ));
                }
            }
        }
        targets.sort_unstable();
        Ok(targets)
    }
}
