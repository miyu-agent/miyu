//! 标准输入、输出、错误原样转（施工 5-9 改上）：MXC 给的是三根管道，助手开线程搬。命令写完、退出以后把输出搬完再退；标准
//! 输入那一根照读的读，命令先退了的不等它（读你的标准输入的那根线程可能一直等着）。

use std::io::{self, Read, Write};
use std::process::ExitCode;
use std::thread;

use mxc_sdk::v1::{MxcProcess, WaitResult};

use miyu_sandbox::EXIT_HELPER;

/// 搬到命令结束，照它的退出码退出：超时（不会有，请求不限时）、等不到的退出 125。
pub(super) fn through(mut process: MxcProcess) -> ExitCode {
    if let Some(mut stdin) = process.take_stdin() {
        thread::spawn(move || {
            #[expect(clippy::let_underscore_must_use, reason = "命令不读了就算了")]
            let _ = io::copy(&mut io::stdin().lock(), &mut stdin);
        });
    }
    let out = process
        .take_stdout()
        .map(|stdout| thread::spawn(move || copy(stdout, io::stdout())));
    let err = process
        .take_stderr()
        .map(|stderr| thread::spawn(move || copy(stderr, io::stderr())));
    let waited = process.wait();
    for copying in [out, err].into_iter().flatten() {
        #[expect(
            clippy::let_underscore_must_use,
            reason = "搬不完的已经丢了，照退出码退"
        )]
        let _ = copying.join();
    }
    match waited {
        Ok(WaitResult::Exited(code)) => std::process::exit(code),
        Ok(other) => crate::fail(&format!("cannot confine: the command ended as {other:?}")),
        Err(error) => {
            crate::say(&format!("cannot wait for the command: {error}"));
            ExitCode::from(EXIT_HELPER)
        }
    }
}

/// 一根管道搬到底，每搬一段就送出去（交互的命令要马上看得到）。
fn copy(mut from: Box<dyn Read + Send>, mut to: impl Write) {
    let mut buffer = [0_u8; 8192];
    while let Ok(read) = from.read(&mut buffer) {
        if read == 0
            || to
                .write_all(&buffer[..read])
                .and_then(|()| to.flush())
                .is_err()
        {
            return;
        }
    }
}
