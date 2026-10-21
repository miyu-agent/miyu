//! 怎么起一个程序（施工 X-12 上，施工 X-34 挪到这里）：有 `#!` 的照写的起；Windows 上照解释器的名字、没有 `#!` 的照扩展名找，
//! 写的是在的绝对路径的照它。

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{Launch, launcher};

/// 程序是 `program`、前面接 `list` 的那一条。
fn launch(program: &str, list: &[&str]) -> Launch {
    Launch {
        program: PathBuf::from(program),
        args: list.iter().map(OsString::from).collect(),
    }
}

#[test]
fn unix_runs_the_shebang_or_the_file_itself() {
    let file = Path::new("/s/weather.py");
    assert_eq!(
        launcher(file, "#!/usr/bin/env python3", false),
        launch("/usr/bin/env", &["python3", "/s/weather.py"])
    );
    assert_eq!(
        launcher(file, "#!/bin/bash -eu", false),
        launch("/bin/bash", &["-eu", "/s/weather.py"])
    );
    assert_eq!(
        launcher(file, "import sys", false),
        launch("/s/weather.py", &[])
    );
    assert_eq!(
        launcher(file, "#!", false),
        launch("/s/weather.py", &[]),
        "#! 后面什么都没有的当没写"
    );
}

#[test]
fn windows_finds_the_interpreter_by_name_or_extension() {
    let py = Path::new(r"C:\s\weather.py");
    assert_eq!(
        launcher(py, "#!/usr/bin/env python3", true),
        launch("python", &[r"C:\s\weather.py"])
    );
    assert_eq!(
        launcher(py, "#!/usr/bin/env node --no-warnings", true),
        launch("node", &["--no-warnings", r"C:\s\weather.py"])
    );
    assert_eq!(
        launcher(py, "#!/bin/bash", true),
        launch("bash", &[r"C:\s\weather.py"])
    );
    assert_eq!(
        launcher(py, "import sys", true),
        launch("python", &[r"C:\s\weather.py"])
    );
    assert_eq!(
        launcher(Path::new("x.PS1"), "# Description: Hi.", true),
        launch(
            "powershell",
            &["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "x.PS1"]
        )
    );
    assert_eq!(
        launcher(Path::new("x.cmd"), "@echo off", true),
        launch("cmd", &["/C", "x.cmd"])
    );
    assert_eq!(
        launcher(Path::new("x.exe"), "MZ", true),
        launch("x.exe", &[])
    );
}

/// Windows 上写的是这台机器上在的绝对路径的照它（施工 X-34）：测试照它找测试用的程序；不在的照名字找。只在 Windows 上跑：
/// 看「在不在」的那一步只在真的 Windows 上做。
#[cfg(windows)]
#[test]
fn windows_uses_an_absolute_interpreter_that_exists() {
    let dir = std::env::temp_dir().join(format!("miyu-launch-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let interpreter = dir.join("fake-python");
    std::fs::write(&interpreter, "").unwrap();
    let first = format!("#!{} -u", interpreter.display());
    let file = Path::new(r"C:\s\hello");
    assert_eq!(
        launcher(file, &first, true),
        Launch {
            program: interpreter.clone(),
            args: vec!["-u".into(), file.as_os_str().to_owned()],
        }
    );
    let gone = format!("#!{}", dir.join("no-such-python").display());
    assert_eq!(
        launcher(file, &gone, true).program,
        PathBuf::from("no-such-python"),
        "不在的照名字找"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
