//! 测试扩展起一个睡着的子进程（施工 XH-1：核心没了，扩展和它起的子进程都跟着走）：`child:<路径>`。测试硬杀核心以后照记下的
//! 两个进程号看它们还在不在。

use std::process::{Command, Stdio};

/// Windows 上不弹控制台窗口：扩展自己是不带控制台起的，子进程不说的话会新开一个。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 认得 `step` 的照做，交回认没认得。`child:<路径>`：照自己的程序起一个 `hang`（不管标准输入，一直睡），三个标准流都接空的
/// （不拿着核心给扩展的管道）；往这个路径写两行：自己的进程号、它的进程号。先写到旁边再改名，测试读到的总是整份。
pub(crate) fn step(step: &str) -> bool {
    let Some(path) = step.strip_prefix("child:") else {
        return false;
    };
    let mut command = Command::new(std::env::current_exe().expect("知道自己在哪"));
    command
        .arg("hang")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[expect(
        clippy::zombie_processes,
        reason = "故意不收：它要一直睡到被看门的整组杀掉，测试照进程号看它还在不在"
    )]
    let child = command.spawn().expect("起得来");
    let partial = format!("{path}.part");
    std::fs::write(
        &partial,
        format!("{}\n{}\n", std::process::id(), child.id()),
    )
    .expect("写得进");
    std::fs::rename(&partial, path).expect("改得了名");
    true
}
