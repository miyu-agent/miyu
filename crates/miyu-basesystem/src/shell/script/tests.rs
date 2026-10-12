//! 脚本怎么起（施工 X-12 上）：有 `#!` 的照写的起；Windows 上照解释器的名字、没有 `#!` 的照扩展名找。

use std::ffi::OsString;
use std::path::Path;

use super::launcher;

fn args(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

#[test]
fn unix_runs_the_shebang_or_the_file_itself() {
    let file = Path::new("/s/weather.py");
    assert_eq!(
        launcher(file, "#!/usr/bin/env python3", false),
        ("/usr/bin/env".into(), args(&["python3", "/s/weather.py"]))
    );
    assert_eq!(
        launcher(file, "#!/bin/bash -eu", false),
        ("/bin/bash".into(), args(&["-eu", "/s/weather.py"]))
    );
    assert_eq!(
        launcher(file, "import sys", false),
        ("/s/weather.py".into(), Vec::new())
    );
    assert_eq!(
        launcher(file, "#!", false),
        ("/s/weather.py".into(), Vec::new()),
        "#! 后面什么都没有的当没写"
    );
}

#[test]
fn windows_finds_the_interpreter_by_name_or_extension() {
    let py = Path::new(r"C:\s\weather.py");
    assert_eq!(
        launcher(py, "#!/usr/bin/env python3", true),
        ("python".into(), args(&[r"C:\s\weather.py"]))
    );
    assert_eq!(
        launcher(py, "#!/usr/bin/env node --no-warnings", true),
        ("node".into(), args(&["--no-warnings", r"C:\s\weather.py"]))
    );
    assert_eq!(
        launcher(py, "#!/bin/bash", true),
        ("bash".into(), args(&[r"C:\s\weather.py"]))
    );
    assert_eq!(
        launcher(py, "import sys", true),
        ("python".into(), args(&[r"C:\s\weather.py"]))
    );
    assert_eq!(
        launcher(Path::new("x.PS1"), "# Description: Hi.", true),
        (
            "powershell".into(),
            args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "x.PS1"])
        )
    );
    assert_eq!(
        launcher(Path::new("x.cmd"), "@echo off", true),
        ("cmd".into(), args(&["/C", "x.cmd"]))
    );
    assert_eq!(
        launcher(Path::new("x.exe"), "MZ", true),
        ("x.exe".into(), Vec::new())
    );
}
