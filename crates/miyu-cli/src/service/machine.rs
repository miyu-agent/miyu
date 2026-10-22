//! 「机器」：`flow.rs` 经它碰这台机器（写文件、跑服务管理器的命令、找家目录和 `miyu` 的位置）。真的那一台在这里，测试里换替身。

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::plan::{Manager, Place};

/// 跑一条命令的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ran {
    /// 退出码是不是 0。
    pub(crate) ok: bool,
    /// 标准错误（没有的是标准输出）的原话，去掉前后空白。
    pub(crate) said: String,
}

/// `flow.rs` 要碰的几样。
pub(crate) trait Machine {
    /// 用哪种服务管理器。
    fn manager(&self) -> Manager;
    /// 这个人的几个目录和身份；不知道家目录的没有。
    fn place(&self) -> Option<Place>;
    /// `miyu` 的真实位置。
    fn exe(&self) -> io::Result<PathBuf>;
    /// `MIYU_HOME`（写成绝对路径的）；没设的没有。
    fn data_root(&self) -> Option<PathBuf>;
    /// 写文件，缺的上级一起建。
    fn write(&self, file: &Path, text: &str) -> io::Result<()>;
    /// 删文件。
    fn remove(&self, file: &Path) -> io::Result<()>;
    /// 文件在不在。
    fn exists(&self, file: &Path) -> bool;
    /// 跑一条命令：程序加参数。
    fn run(&self, command: &[String]) -> Ran;
}

/// 这台机器。
pub(crate) struct Real;

impl Machine for Real {
    fn manager(&self) -> Manager {
        if cfg!(target_os = "macos") {
            return Manager::Launchd;
        }
        if cfg!(windows) {
            return Manager::Task;
        }
        // 有 systemd 用户会话的：它答得上自己的环境。
        let systemd = ["systemctl", "--user", "show-environment"].map(String::from);
        match self.run(&systemd).ok {
            true => Manager::Systemd,
            false => Manager::Autostart,
        }
    }

    fn place(&self) -> Option<Place> {
        let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
        let home = PathBuf::from(var(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?);
        let config = var("XDG_CONFIG_HOME").map(PathBuf::from);
        #[cfg(unix)]
        let uid = std::fs::metadata(&home)
            .ok()
            .map(|metadata| std::os::unix::fs::MetadataExt::uid(&metadata));
        #[cfg(not(unix))]
        let uid = None;
        let user = match (var("USERDOMAIN"), var("USERNAME")) {
            (Some(domain), Some(name)) if cfg!(windows) => Some(format!(
                "{}\\{}",
                domain.to_string_lossy(),
                name.to_string_lossy()
            )),
            _ => None,
        };
        Some(Place {
            home,
            config,
            uid,
            user,
        })
    }

    fn exe(&self) -> io::Result<PathBuf> {
        std::env::current_exe()?.canonicalize()
    }

    fn data_root(&self) -> Option<PathBuf> {
        let root = std::env::var_os("MIYU_HOME").filter(|value| !value.is_empty())?;
        std::path::absolute(PathBuf::from(root)).ok()
    }

    fn write(&self, file: &Path, text: &str) -> io::Result<()> {
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(file, text)
    }

    fn remove(&self, file: &Path) -> io::Result<()> {
        std::fs::remove_file(file)
    }

    fn exists(&self, file: &Path) -> bool {
        file.is_file()
    }

    fn run(&self, command: &[String]) -> Ran {
        let Some((program, args)) = command.split_first() else {
            return Ran {
                ok: false,
                said: String::new(),
            };
        };
        let output = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .output();
        match output {
            Ok(output) => {
                let said = match output.stderr.is_empty() {
                    true => output.stdout,
                    false => output.stderr,
                };
                Ran {
                    ok: output.status.success(),
                    said: String::from_utf8_lossy(&said).trim().to_string(),
                }
            }
            Err(error) => Ran {
                ok: false,
                said: error.to_string(),
            },
        }
    }
}
