//! `miyu pkg pack` 给人看的字（施工 G-1；CL-2 补从 `pkg.rs` 挪出来）。

use crate::diagnostic::Problem;
use crate::language::Language;

/// `miyu pkg pack` 的字（施工 G-1）。
impl Language {
    /// 打好了，写在 `path`。
    pub(crate) fn packed(&self, path: &str) -> String {
        match self {
            Language::Chinese => format!("已打包 {path}"),
            Language::English => format!("packed {path}"),
        }
    }

    /// 包文件多大。
    pub(crate) fn packed_size(&self, size: u64) -> String {
        match self {
            Language::Chinese => format!("大小：{size} 字节"),
            Language::English => format!("Size: {size} bytes"),
        }
    }

    /// 包的图标超了上限，打进去的是缩好的那一份（施工 F-18 上）：源文件夹不动。
    pub(crate) fn logo_shrunk(
        &self,
        logo: &str,
        (width, height, size): (u32, u32, usize),
    ) -> String {
        match self {
            Language::Chinese => {
                format!("已把图标 {logo} 缩小到 {width}×{height}，{size} 字节（源文件未改动）")
            }
            Language::English => format!(
                "shrank logo {logo} to {width}×{height}, {size} bytes (source file unchanged)"
            ),
        }
    }

    /// 包的图标打不进去（施工 F-18 上）：不在、不是 PNG / WebP、缩不到上限以内。
    pub(crate) fn bad_logo(&self, logo: &str, why: LogoWhy) -> Problem {
        match (self, why) {
            (Language::Chinese, LogoWhy::Missing) => {
                Problem::new(format!("无法打包：找不到图标 {logo}"))
            }
            (Language::English, LogoWhy::Missing) => {
                Problem::new(format!("cannot pack: logo {logo} not found"))
            }
            (Language::Chinese, LogoWhy::NotImage) => {
                Problem::new(format!("无法打包：图标 {logo} 不是 PNG 或 WebP 图片"))
            }
            (Language::English, LogoWhy::NotImage) => Problem::new(format!(
                "cannot pack: logo {logo} is not a PNG or WebP image"
            )),
            (Language::Chinese, LogoWhy::TooBig) => Problem::new(format!(
                "无法打包：图标 {logo} 无法缩小到 256 KiB 和 512×512 以内"
            ))
            .hint("换一张更简单的图片"),
            (Language::English, LogoWhy::TooBig) => Problem::new(format!(
                "cannot pack: logo {logo} cannot be shrunk below 256 KiB and 512×512"
            ))
            .hint("use a simpler image"),
        }
    }

    /// 文件夹名不是包的编号。
    pub(crate) fn not_a_package_id(&self, name: &str) -> Problem {
        match self {
            Language::Chinese => {
                Problem::new(format!("无法打包：文件夹名 '{name}' 不是有效的软件包编号"))
            }
            Language::English => Problem::new(format!(
                "cannot pack: folder name '{name}' is not a valid package id"
            )),
        }
    }

    /// 读不了清单。
    pub(crate) fn unreadable(&self, path: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("无法读取 {path}")),
            Language::English => Problem::new(format!("cannot read {path}")),
        }
    }

    /// 清单写错了、打不成。
    pub(crate) fn cannot_pack(&self, why: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("无法打包：{why}")),
            Language::English => Problem::new(format!("cannot pack: {why}")),
        }
    }

    /// 清单没写版本。
    pub(crate) fn no_version(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("无法打包：清单没有写版本")
                .hint("在 package.toml 的 [package] 里写上 version"),
            Language::English => Problem::new("cannot pack: the manifest has no version")
                .hint("add version to [package] in package.toml"),
        }
    }

    /// 清单没写 `targets`，却给了 `--target`。
    pub(crate) fn no_targets(&self) -> Problem {
        match self {
            Language::Chinese => {
                Problem::new("无法打包：清单没有写 targets").hint("去掉 '--target'")
            }
            Language::English => {
                Problem::new("cannot pack: the manifest lists no targets").hint("drop '--target'")
            }
        }
    }

    /// 清单写了 `targets`，要用 `--target` 挑一种。
    pub(crate) fn needs_target(&self, listed: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("无法打包：需要指定目标系统（{listed}）"))
                .hint("用 '--target <系统>' 选一个"),
            Language::English => Problem::new(format!("cannot pack: target required ({listed})"))
                .hint("choose one with '--target <target>'"),
        }
    }

    /// `--target` 不在清单的 `targets` 里。
    pub(crate) fn not_listed_target(&self, asked: &str, listed: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!(
                "无法打包：清单的 targets 里没有 {asked}（{listed}）"
            )),
            Language::English => Problem::new(format!(
                "cannot pack: {asked} is not in the manifest's targets ({listed})"
            )),
        }
    }
}

/// 包的图标为什么打不进去（施工 F-18 上）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogoWhy {
    /// 包文件夹里没有。
    Missing,
    /// 不是 PNG、WebP。
    NotImage,
    /// 缩不到上限以内。
    TooBig,
}
