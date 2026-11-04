//! 照规格把一条命令包成交给助手的样子（`docs/blueprint/sandbox.md`「怎么走」第 3 条）：
//! `<助手> run --spec <规格的 JSON> -- <程序> <参数…>`。工作目录、环境变量、标准输入输出由起命令的一方照旧设。
//!
//! Linux 上只有 Landlock 的那一路（施工 V-47）：[`command`] 带上核心按规格缓存的规则集，参数多一段 `--ruleset <号码>`，
//! 只有这个子进程拿得到那个描述符。别的平台、别的规格和 [`argv`] 拼的一样。

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Spec;

/// 一次调用带的沙盒：助手在哪，规格是什么，要给命令设的环境变量（`docs/blueprint/tools/interface.md`「一次调用交给
/// 工具的」）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sandboxed {
    /// 助手 `miyu-sandbox` 的路径。
    pub helper: PathBuf,
    /// 规格。
    pub spec: Spec,
    /// 要给命令设的环境变量，照核心挑好的之后设、同名的盖掉（施工 5-4 上）：例如沙盒自己的临时目录的 `TMPDIR`。
    pub env: Vec<(OsString, OsString)>,
    /// 这份规格每次调用都不一样，不进核心按规格缓存的规则集（施工 V-52）：例如脚本工具每次调用一个新的交图目录放进 `write`，
    /// 缓存对它从来认不上，还会把别的挤出去。为真的 [`command`] 照施工 V-47 以前那样让助手从头建（参数里没有
    /// `--ruleset`），[`checks_files`] 为假；能读能写的范围、报的错和不缓存的一样。只在核心里用，不进助手的参数。别的调用方
    /// 写假，照旧缓存。
    pub uncached: bool,
}

/// 要执行的程序 `program` 和参数 `args`，包成交给助手的：交回助手的路径和给它的参数。
///
/// # Errors
///
/// 规格写不成 JSON：路径不是 UTF-8 的。
pub fn argv(
    sandboxed: &Sandboxed,
    program: &Path,
    args: &[OsString],
) -> Result<(PathBuf, Vec<OsString>), serde_json::Error> {
    let mut out: Vec<OsString> = vec![
        "run".into(),
        "--spec".into(),
        sandboxed.spec.to_json()?.into(),
        "--".into(),
        program.as_os_str().to_os_string(),
    ];
    out.extend(args.iter().cloned());
    Ok((sandboxed.helper.clone(), out))
}

/// 同 [`argv`]，拼成能直接起的命令（施工 V-47）：Linux 上规格能缓存（只用 Landlock、有藏起来的、路径都是绝对的）、核心建得成
/// 规则集的，参数是 `run --spec <规格的 JSON> --ruleset <号码> -- <程序> <参数…>`，只有起的这个子进程拿得到那个规则集；
/// 别的（包括标了 [`Sandboxed::uncached`] 的，施工 V-52）和 [`argv`] 拼的一样。工作目录、环境变量、标准输入输出由起命令的一方照旧设。
///
/// # Errors
///
/// 规格写不成 JSON：路径不是 UTF-8 的。
pub fn command(
    sandboxed: &Sandboxed,
    program: &Path,
    args: &[OsString],
) -> Result<Command, serde_json::Error> {
    let (helper, argv) = argv(sandboxed, program, args)?;
    let mut command = Command::new(helper);
    #[cfg(target_os = "linux")]
    if !sandboxed.uncached
        && let Some(ruleset) = crate::linux::ruleset(&sandboxed.spec)
    {
        let raw = crate::linux::inherit(&mut command, ruleset);
        let (head, tail) = argv.split_at(3);
        command
            .args(head)
            .arg("--ruleset")
            .arg(raw.to_string())
            .args(tail);
        return Ok(command);
    }
    command.args(argv);
    Ok(command)
}

/// [`command`] 会不会在调的线程里看文件系统（施工 V-50）：Linux 上能缓存的规格（只用 Landlock、有藏起来的、路径都是绝对的，
/// 这台机器的 Landlock 能用）每次都要照着规格重新看一遍、和缓存的指纹比（列目录、逐条 `fstatat`、读挂载表），约 2 ms，要重建
/// 的那一次约 5 ms。异步的调用方照它把 [`command`] 挪到阻塞线程里调，造好的命令拿回去照旧起。别的规格、标了
/// [`Sandboxed::uncached`] 的（施工 V-52）、别的平台只拼参数。
#[must_use]
pub fn checks_files(sandboxed: &Sandboxed) -> bool {
    #[cfg(target_os = "linux")]
    return !sandboxed.uncached && crate::linux::looks(&sandboxed.spec);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = sandboxed;
        false
    }
}

#[cfg(test)]
mod tests;
