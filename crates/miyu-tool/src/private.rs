//! 私密目录可配（施工 5-15 下，`docs/designs/11-权限与沙盒.md` 第四节「设置页「隐私与安全」」）：`sandbox.private.add`（人加的）、
//! `sandbox.private.remove`（去掉的出厂项）。记两份、不记整份：以后的版本加了出厂项照样生效，人去掉的不会随升级回来。
//!
//! 「去掉的出厂项」能写的就是这台机器的出厂清单（[`miyu_fs::SHIPPED_PRIVATE`]，照系统不同）。清单里登记成文字的列表：生成的
//! Schema、参考文件哪个系统都一样（写成选项的话 macOS、Windows 上和样本对不上，CI 撞见）；出厂清单由端点在 `config.schema` 里当
//! 选项交给头、`config.set` 时查（施工 5-15 下补）。两项手写，同 `settings!` 以外的写法一致。权限策略、沙盒、往下走目录的工具、头浏览文件都照这一轮（头的照这一刻）的配置拼（[`dirs`]）。

use std::path::{Path, PathBuf};

use miyu_config::{Applies, Control, Item, Kind, Layer, Setting, Ui, Values};

/// 私密目录整个开不开（施工 5-17 上，2026-10-21 项目主人定，出厂开）：关了出厂的、人加的都不藏、文件工具不问；数据根照旧藏
/// （沙盒、权限策略另算，设计 11 第五节：数据根是 Miyu 自己的状态和密钥）。
pub const ENABLED: &str = "sandbox.private.enabled";

/// 人加的私密目录。
pub const ADD: &str = "sandbox.private.add";

/// 去掉的出厂项。
pub const REMOVE: &str = "sandbox.private.remove";

/// 一个目录的写法最多几个字。
const LONGEST: usize = 4096;

/// 设置页「隐私与安全」那一页、私密目录那一组。
const UI: Ui = Ui {
    page: "privacy",
    group: "private",
    package: None,
    common: false,
    control: Control::List,
    hidden: false,
};

/// 这三项：系统、个人两层，下一轮生效；开关出厂开，两份目录没写的是空的。开关排在组里最上面。
pub const ITEMS: &[Item] = &[
    Item {
        key: ENABLED,
        kind: Kind::Bool,
        default: Some(miyu_config::Value::Bool(true)),
        layers: &[Layer::System],
        tighten: None,
        env: None,
        applies: Applies::NextTurn,
        ui: Ui {
            control: Control::Toggle,
            ..UI
        },
    },
    Item {
        key: ADD,
        kind: Kind::List(&Kind::Text { max: LONGEST }),
        default: None,
        layers: &[Layer::System],
        tighten: None,
        env: None,
        applies: Applies::NextTurn,
        ui: UI,
    },
    Item {
        key: REMOVE,
        kind: Kind::List(&Kind::Text { max: LONGEST }),
        default: None,
        layers: &[Layer::System],
        tighten: None,
        env: None,
        applies: Applies::NextTurn,
        ui: UI,
    },
];

/// 照最终值 `values` 拼出家目录 `home` 下这一刻的私密目录：出厂的去掉人去掉的，加上人加的，出厂项跟着核心进程的环境变量挪去的
/// 也算（[`miyu_fs::private_dirs_with`]）；关了的（[`ENABLED`]）是空的。
pub fn dirs(values: &Values, home: &Path) -> Vec<PathBuf> {
    dirs_in(values, home, &|name| std::env::var_os(name))
}

/// 同 [`dirs`]，环境变量照 `env` 读（测试里不碰这台机器的环境）。
pub fn dirs_in(
    values: &Values,
    home: &Path,
    env: &dyn Fn(&str) -> Option<std::ffi::OsString>,
) -> Vec<PathBuf> {
    if <Option<bool>>::read(values.get(ENABLED)) == Some(false) {
        return Vec::new();
    }
    let read = |key: &str| <Option<Vec<String>>>::read(values.get(key)).unwrap_or_default();
    miyu_fs::private_dirs_with(home, &read(ADD), &read(REMOVE), env)
}

#[cfg(test)]
mod tests;
