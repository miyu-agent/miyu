//! Windows 的命令行（施工 5-9 改上）：MXC 收的是一整行字，程序和参数照 `CommandLineToArgvW`、MSVC 运行库的规矩拼回去，那一头
//! 拆出来和这里交的一模一样。照 Rust 标准库在 Windows 上起进程时的拼法（`std::sys::args::windows`）：带空白、制表符、双引号、
//! 或者是空的才加引号；引号里的双引号前面加反斜杠，紧挨着双引号、结尾的反斜杠成倍。纯算字，哪个系统上都能测。

use std::ffi::OsStr;

/// 把程序 `program`、参数 `args` 拼成一行。有一段不是合法的 Unicode（Windows 上是落单的代理项）的没有：MXC 收的是 `String`。
pub fn windows_command_line(program: &OsStr, args: &[impl AsRef<OsStr>]) -> Option<String> {
    let mut line = String::new();
    quote(&mut line, program.to_str()?);
    for arg in args {
        line.push(' ');
        quote(&mut line, arg.as_ref().to_str()?);
    }
    Some(line)
}

/// 一段照规矩写进 `line`。
fn quote(line: &mut String, arg: &str) {
    let needs = arg.is_empty() || arg.contains([' ', '\t', '"']);
    if !needs {
        line.push_str(arg);
        return;
    }
    line.push('"');
    let mut backslashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                // 双引号前面的反斜杠成倍，再给双引号自己加一个。
                line.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                backslashes = 0;
            }
            _ => {
                line.extend(std::iter::repeat_n('\\', backslashes));
                backslashes = 0;
            }
        }
        if c != '\\' {
            line.push(c);
        }
    }
    // 结尾的反斜杠挨着收尾的双引号：成倍。
    line.extend(std::iter::repeat_n('\\', backslashes * 2));
    line.push('"');
}

#[cfg(test)]
mod tests;
