//! 怎么起一个程序（施工 X-34，`docs/blueprint/packages.md`「转交」，原来在脚本工具里，施工 X-12 上）：包里的程序、脚本工具
//! 共用这一份。交回 [`Launch`]：程序，和接在别的参数前面的几个。
//!
//! - 有 `#!` 的：Unix 上照写的起（解释器、它后面那几个参数、文件）；Windows 上解释器写的是这台机器上在的绝对路径的照它，
//!   别的取解释器的名字（`env` 的取它后面那个，`python3` 换成 `python`），照 `PATH` 找。
//! - 没有的：Unix 上直接起文件；Windows 上照扩展名：`.py` 的 `python`，`.ps1` 的 PowerShell（不读配置、不管执行策略），`.js` 的
//!   `node`，`.cmd`、`.bat` 的 `cmd /C`，别的直接起。

use std::ffi::OsString;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

/// 起一个程序的那一条命令：程序，和接在别的参数前面的几个（照 `#!` 起的是解释器后面写的那几个和文件本身）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    /// 程序：路径，或者照 `PATH` 找的名字。
    pub program: PathBuf,
    /// 接在前面的参数。
    pub args: Vec<OsString>,
}

impl Launch {
    /// 直接起 `program`，前面不接参数。
    #[must_use]
    pub fn direct(program: PathBuf) -> Launch {
        Launch {
            program,
            args: Vec::new(),
        }
    }

    /// 照它造一条命令：程序、前面的参数放好，别的由调用的一方接着加。
    #[must_use]
    pub fn command(&self) -> std::process::Command {
        let mut command = std::process::Command::new(&self.program);
        command.args(&self.args);
        command
    }
}

/// 文件 `file` 的第一行，去掉行尾的空白。
///
/// # Errors
///
/// 打不开、读不了。
pub fn first_line(file: &Path) -> io::Result<String> {
    let mut line = String::new();
    io::BufReader::new(std::fs::File::open(file)?).read_line(&mut line)?;
    Ok(line.trim_end().to_string())
}

/// 起文件 `file` 的那一条命令，它的第一行是 `first`；`windows` 是 Windows 上的找法。
pub fn launcher(file: &Path, first: &str, windows: bool) -> Launch {
    let script = file.as_os_str().to_owned();
    if let Some(rest) = first.strip_prefix("#!") {
        let mut words = rest.split_whitespace();
        if let Some(interpreter) = words.next() {
            let extra: Vec<&str> = words.collect();
            let program = |program: PathBuf, before: &[&str]| {
                let mut args: Vec<OsString> = before.iter().map(OsString::from).collect();
                args.push(script.clone());
                Launch { program, args }
            };
            // Unix 上照写的起；Windows 上写的是这台机器上在的绝对路径的也照它（施工 X-34）。看「在不在」只在真的 Windows 上：
            // 别的平台上照 Windows 的找法走（测试），`/usr/bin/env` 这类在本机是在的，不能当 Windows 的绝对路径。
            let written = Path::new(interpreter);
            if !windows || (cfg!(windows) && written.is_absolute() && written.is_file()) {
                return program(written.to_path_buf(), &extra);
            }
            let name = written.file_name().map_or_else(
                || interpreter.to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            let (found, rest) = match (name.as_str(), extra.split_first()) {
                ("env", Some((found, rest))) => ((*found).to_string(), rest.to_vec()),
                _ => (name, extra),
            };
            // Windows 上的 Python 叫 `python`：`python3` 常常是应用商店的占位程序。
            let found = if found == "python3" {
                "python".to_string()
            } else {
                found
            };
            return program(PathBuf::from(found), &rest);
        }
    }
    if !windows {
        return Launch::direct(file.to_path_buf());
    }
    let extension = file
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase());
    let (program, before): (&str, &[&str]) = match extension.as_deref() {
        Some("py") => ("python", &[]),
        Some("ps1") => (
            "powershell",
            &["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"],
        ),
        Some("js") => ("node", &[]),
        Some("cmd" | "bat") => ("cmd", &["/C"]),
        _ => return Launch::direct(file.to_path_buf()),
    };
    let mut args: Vec<OsString> = before.iter().map(OsString::from).collect();
    args.push(script);
    Launch {
        program: PathBuf::from(program),
        args,
    }
}

#[cfg(test)]
mod tests;
