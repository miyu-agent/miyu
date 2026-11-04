//! 沙盒的助手 `miyu-sandbox`（`docs/blueprint/sandbox.md`「助手的命令行」）：照规格先把自己收紧，再换成那条命令。
//!
//! - `miyu-sandbox run --spec <规格的 JSON> -- <程序> [参数…]`：Unix 上换成它（`exec`），Windows 上起它、等它。
//! - `miyu-sandbox probe`：印一行 JSON，说这台机器能收紧到什么程度。
//! - `miyu-sandbox lifeline`（Unix，施工 V-48）：看门，读到标准输入的结尾就杀掉自己所在的整个组（`lifeline.rs`）。
//!
//! 这个文件只管各平台共用的：读参数、读规格、印探测的结果、出错时说的那几句。收紧和换成命令每个平台一个文件
//! （`linux.rs`、`macos.rs`、`windows.rs`，别的系统 `other.rs`），各平台的施工各改各的；每个都交出同样的两样：
//! `run`（收紧，再换成命令）、`mechanisms`（探测时报的手段）。Linux（施工 5-2、5-3）、macOS（5-7）上照规格收紧，Windows
//! 还不收紧（5-9）。
//!
//! 成了什么都不印：它的标准错误就是命令的标准错误，印了会混进给她看的输出。出错印一句英文，退出码照 `env`、`timeout`
//! 的约定（[`EXIT_HELPER`]、[`EXIT_CANNOT_RUN`]、[`EXIT_NOT_FOUND`]）。

#[cfg(unix)]
mod lifeline;
#[cfg(target_os = "linux")]
mod linux;
// 别的 Unix 上跑测试时也编进去：写配置、换成真实位置的那两块不碰 macOS 的接口，单元测试在 Linux 上也跑（施工 5-7）。
#[cfg(any(target_os = "macos", all(test, unix)))]
mod macos;
#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
mod other;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
use other as platform;
#[cfg(windows)]
use windows as platform;

use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

use miyu_sandbox::{EXIT_HELPER, Probe, Spec};

/// 参数不对时说的。
const USAGE: &str = "usage: miyu-sandbox run --spec <json> -- <program> [args...]";

fn main() -> ExitCode {
    // 看门的最先认（施工 V-48）：每条命令起一个，别的什么都不做。
    #[cfg(unix)]
    {
        let mut args = std::env::args_os();
        if args.len() == 2
            && args
                .nth(1)
                .is_some_and(|mode| mode == miyu_sandbox::lifeline::MODE)
        {
            return lifeline::watch();
        }
    }
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match args.split_first() {
        Some((first, [])) if first == "probe" => probe(),
        Some((first, rest)) if first == "run" => run(rest),
        _ => fail(USAGE),
    }
}

/// `run` 后面的：`--spec <JSON> [--ruleset <号码>] -- <程序> [参数…]`。`--ruleset` 是核心交来的缓存的规则集（施工 V-47，只有
/// Linux 上只用 Landlock 时有），别的平台给了算参数不对。
fn run(args: &[OsString]) -> ExitCode {
    let (ruleset, args) = match args {
        [flag, spec, ruled, number, rest @ ..] if ruled == "--ruleset" => {
            match number
                .to_str()
                .and_then(|number| number.parse::<i32>().ok())
            {
                Some(number) if number >= 3 => (
                    Some(number),
                    [flag, spec].into_iter().chain(rest).cloned().collect(),
                ),
                _ => return fail(USAGE),
            }
        }
        _ => (None, args.to_vec()),
    };
    let [flag, spec, dashes, program, rest @ ..] = args.as_slice() else {
        return fail(USAGE);
    };
    if flag != "--spec" || dashes != "--" {
        return fail(USAGE);
    }
    let spec = match spec.to_str().map(Spec::from_json) {
        Some(Ok(spec)) => spec,
        Some(Err(error)) => return fail(&format!("bad spec: {error}")),
        None => return fail("bad spec: not UTF-8"),
    };
    // 关不住读的平台收到只准读一部分的规格：不跑，宁可不跑也不漏读（施工 5-12 下）。
    if spec.read.is_some() && !miyu_sandbox::CONFINES_READS {
        return fail("cannot confine: reads cannot be confined on this platform");
    }
    match ruleset {
        None => platform::run(&spec, program, rest),
        Some(ruleset) => ruled(&spec, ruleset, program, rest),
    }
}

/// 照核心交来的规则集收紧（施工 V-47）：只有 Linux 有。
#[cfg(target_os = "linux")]
fn ruled(spec: &Spec, ruleset: i32, program: &std::ffi::OsStr, rest: &[OsString]) -> ExitCode {
    platform::run_ruled(spec, ruleset, program, rest)
}

/// 同上：别的平台没有，算参数不对。
#[cfg(not(target_os = "linux"))]
fn ruled(_spec: &Spec, _ruleset: i32, _program: &std::ffi::OsStr, _rest: &[OsString]) -> ExitCode {
    fail(USAGE)
}

/// 执行不了：找不到的 127，别的 126。
fn cannot_run(program: &std::ffi::OsStr, error: &io::Error) -> ExitCode {
    say(&format!(
        "cannot run {}: {error}",
        program.to_string_lossy()
    ));
    ExitCode::from(if error.kind() == io::ErrorKind::NotFound {
        miyu_sandbox::EXIT_NOT_FOUND
    } else {
        miyu_sandbox::EXIT_CANNOT_RUN
    })
}

/// 印这台机器的探测结果，一行。
fn probe() -> ExitCode {
    let written = serde_json::to_string(&Probe::new(platform::mechanisms()))
        .map_err(io::Error::other)
        .and_then(|line| {
            let mut out = io::stdout().lock();
            writeln!(out, "{line}").and_then(|()| out.flush())
        });
    match written {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => fail(&format!("cannot write the probe: {error}")),
    }
}

/// 助手自己出了错：说一句，退出 125。收紧不成的，各平台照 `cannot confine: <原话>` 说（蓝图定的写法）。
fn fail(what: &str) -> ExitCode {
    say(what);
    ExitCode::from(EXIT_HELPER)
}

/// 在标准错误上说一句，带上自己的名字：混在命令的输出里也认得出是谁说的。
fn say(what: &str) {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "标准错误写不了的，没有别的地方可以说"
    )]
    let _ = writeln!(io::stderr().lock(), "miyu-sandbox: {what}");
}
