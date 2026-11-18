//! 交给浏览器的命令（施工 AU-014）。

use super::opener;

/// 修以前 Windows 上是 `cmd /C start "" <网址>`，`&` 后面的被 `cmd` 当成另一条命令（审计 #014）。哪个系统都不经过 shell，网址
/// 整个是最后一个参数。
#[test]
fn the_url_is_one_argument_and_no_shell_reads_it() {
    let url = "http://127.0.0.1:8770/?code=a&lang=zh|x%PATH%";
    let command = opener(url);
    let program = command.get_program().to_string_lossy().to_ascii_lowercase();
    for shell in ["cmd", "cmd.exe", "sh", "bash", "powershell", "pwsh"] {
        assert_ne!(program, shell);
    }
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(
        args.last().map(|arg| arg.to_string_lossy()),
        Some(url.into())
    );
    if cfg!(windows) {
        assert_eq!(program, "rundll32.exe");
        assert_eq!(args.len(), 2);
    } else {
        assert_eq!(args.len(), 1);
    }
}
