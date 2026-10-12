//! `miyu-extract`：知识库抽字的小程序，用法见库的说明（[`miyu_extract`]）。

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let (written, code) = match miyu_extract::run(&args) {
        Ok(json) => (
            writeln!(std::io::stdout().lock(), "{json}"),
            ExitCode::SUCCESS,
        ),
        Err(why) => (
            writeln!(std::io::stderr().lock(), "{why}"),
            ExitCode::FAILURE,
        ),
    };
    // 写不出去（核心那头关了管道）也只能退：没人收了。
    match written {
        Ok(()) => code,
        Err(_) => ExitCode::FAILURE,
    }
}
