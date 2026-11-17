//! 包支持哪几种系统（施工 PL-2，设计 `37-软件源.md` 第二节第 3 条补，项目主人 2026-11-17 定）：清单 `[package] systems`
//! 写的，只说操作系统（`linux`、`macos`、`windows`），不说处理器。没写的哪种系统都支持。
//!
//! 和 `targets`（[`super::Target`]）分开：`targets` 是「一种系统一个包文件」（带本机程序的包才写），`systems` 是作者支持
//! 哪几种系统（脚本、扩展不带程序也能写，例如只做了 Linux、macOS 的）。两样都写的，`targets` 里每一种的系统都要在 `systems`
//! 里，不然 `bad_systems`。本机支不支持照两样一起算（[`fits_here`]）：没写 `systems` 的照 `targets` 推（有本机这一种包文件
//! 才算支持），两样都没写的哪里都支持。「平台」在清单里说的是通讯平台（`[connection] platform`），这里照 `targets` 叫「系统」。

use toml_edit::TableLike;

use super::reader::Reader;
use super::{Code, Problem, Target};

/// 一种操作系统。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum System {
    /// Linux。
    Linux,
    /// macOS。
    Macos,
    /// Windows。
    Windows,
}

impl System {
    /// 全部，照清单里的先后。
    pub const ALL: [System; 3] = [System::Linux, System::Macos, System::Windows];

    /// 清单、索引里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            System::Linux => "linux",
            System::Macos => "macos",
            System::Windows => "windows",
        }
    }

    /// 照写法认；不认识的没有。
    pub fn parse(text: &str) -> Option<System> {
        System::ALL
            .into_iter()
            .find(|system| system.as_str() == text)
    }

    /// 这个程序编出来跑在哪一种上；不在这三种里的没有。编译时定的，不读环境。
    pub fn here() -> Option<System> {
        if cfg!(target_os = "linux") {
            Some(System::Linux)
        } else if cfg!(target_os = "macos") {
            Some(System::Macos)
        } else if cfg!(target_os = "windows") {
            Some(System::Windows)
        } else {
            None
        }
    }
}

impl Target {
    /// 这一种是哪个操作系统。
    pub fn system(self) -> System {
        match self {
            Target::LinuxX86_64 | Target::LinuxAarch64 => System::Linux,
            Target::MacosAarch64 => System::Macos,
            Target::WindowsX86_64 => System::Windows,
        }
    }
}

/// 写了 `systems`、`targets` 的包在本机（系统 `system`、操作系统加处理器 `target`）支不支持：`systems` 写了的要有本机的系统，
/// `targets` 写了的要有本机这一种；没写的那一样不管。本机不在认得的那几种里的，写了就不支持。
pub fn fits_here(
    systems: &[System],
    targets: &[Target],
    (system, target): (Option<System>, Option<Target>),
) -> bool {
    (systems.is_empty() || system.is_some_and(|here| systems.contains(&here)))
        && (targets.is_empty() || target.is_some_and(|here| targets.contains(&here)))
}

impl Reader<'_> {
    /// `[package] systems`：字的列表，每个都是认识的系统、不重复；照 [`System::ALL`] 的先后排好。没写的是空的。`targets`
    /// 已经读好的，每一种的系统都要在里面。
    pub(super) fn systems(
        &self,
        table: &dyn TableLike,
        targets: &[Target],
    ) -> Result<Vec<System>, Problem> {
        let mut systems = Vec::new();
        let refuse = |detail: &str, why: &str| {
            self.problem(
                table.get("systems"),
                Code::BadSystems,
                detail,
                format!("package.systems: \"{detail}\" {why}"),
            )
        };
        for name in &self.texts(table, "package", "systems")? {
            match System::parse(name) {
                Some(system) if !systems.contains(&system) => systems.push(system),
                Some(_) => return Err(refuse(name, "is listed twice")),
                None => {
                    return Err(refuse(
                        name,
                        "is not a known system (linux, macos, windows)",
                    ));
                }
            }
        }
        if !systems.is_empty()
            && let Some(target) = targets
                .iter()
                .find(|target| !systems.contains(&target.system()))
        {
            return Err(refuse(
                target.as_str(),
                "is in package.targets but its system is not in package.systems",
            ));
        }
        systems.sort_unstable();
        Ok(systems)
    }
}

#[cfg(test)]
mod tests;
