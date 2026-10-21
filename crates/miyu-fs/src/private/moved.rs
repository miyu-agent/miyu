//! 跟着环境变量挪的出厂项（施工 5-17 上，2026-10-21 项目主人定，照 sandbox-runtime 的 `ENV_STORES`）：工具照环境变量把凭据放到
//! 别处的，新位置也藏，和它对应的出厂项一起算（那一项去掉了的，挪去的也不藏）。变量照核心进程的环境读；不是绝对路径的、家
//! 目录本身和它的上级不算。

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::too_broad;

/// 变量怎么换成位置。
#[derive(Clone, Copy)]
enum Moves {
    /// 变量就是那个文件或者目录。
    Itself,
    /// 变量是几个路径（照系统的分隔符分开，`KUBECONFIG` 这种）。
    List,
    /// 变量是一个目录，凭据是它下面这个文件（`CARGO_HOME` 下的 `credentials.toml`）。
    Under(&'static str),
}

/// 出厂项、变量、怎么换。
const MOVED: &[(&str, &str, Moves)] = &[
    ("~/.gnupg", "GNUPGHOME", Moves::Itself),
    ("~/.aws", "AWS_SHARED_CREDENTIALS_FILE", Moves::Itself),
    ("~/.aws", "AWS_CONFIG_FILE", Moves::Itself),
    ("~/.kube", "KUBECONFIG", Moves::List),
    ("~/.docker", "DOCKER_CONFIG", Moves::Itself),
    ("~/.config/gh", "GH_CONFIG_DIR", Moves::Itself),
    (
        "~/.cargo/credentials.toml",
        "CARGO_HOME",
        Moves::Under("credentials.toml"),
    ),
    ("~/.azure", "AZURE_CONFIG_DIR", Moves::Itself),
    ("~/.netrc", "NETRC", Moves::Itself),
];

/// 出厂项 `shipped` 跟着环境变量挪去的位置：`env` 照变量名交值。`~/.config/` 下的照 `XDG_CONFIG_HOME` 接上后面那一截。
pub(super) fn moved(
    home: &Path,
    shipped: &str,
    env: &dyn Fn(&str) -> Option<OsString>,
) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for (item, name, into) in MOVED {
        if *item != shipped {
            continue;
        }
        let Some(value) = env(name).filter(|value| !value.is_empty()) else {
            continue;
        };
        match into {
            Moves::Itself => found.push(PathBuf::from(value)),
            Moves::List => found.extend(std::env::split_paths(&value)),
            Moves::Under(file) => found.push(PathBuf::from(value).join(file)),
        }
    }
    if let (Some(rest), Some(config)) = (
        shipped.strip_prefix("~/.config/"),
        env("XDG_CONFIG_HOME").filter(|value| !value.is_empty()),
    ) {
        let joined = rest
            .split('/')
            .filter(|part| !part.is_empty())
            .fold(PathBuf::from(config), |dir, part| dir.join(part));
        found.push(joined);
    }
    found.retain(|path| path.is_absolute() && !too_broad(home, path));
    found
}
