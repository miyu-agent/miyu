use std::fs::File;
use std::os::windows::io::AsRawHandle;

use windows_sys::Win32::Foundation::{
    GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation,
};
use windows_sys::Win32::System::Console::{STD_INPUT_HANDLE, SetStdHandle};

use super::*;

/// 句柄 `handle` 的继承标记。
fn inheritable(handle: HANDLE) -> bool {
    let mut flags = 0;
    // SAFETY: `handle` 是测试自己开着的文件的句柄，`flags` 是本函数里的变量。
    let read = unsafe { GetHandleInformation(handle, &mut flags) };
    assert_ne!(read, 0, "读得到句柄的标记");
    flags & HANDLE_FLAG_INHERIT != 0
}

/// 测试改过的进程级状态，走的时候（断言失败也一样）原样放回（施工 FL-6）：三个标准句柄是哪一个、各自的继承标记。标准句柄表
/// 是整个测试进程共用的：换成测试的文件以后不放回，文件一关，标准输入就指着一个关掉的句柄，同一个测试程序里后面再起子进程
/// （标准库照标准输入复制一份给它）就起不来（Windows CI 上 `parent` 那条测试红过：`os error 50`）。
struct Restore {
    saved: Vec<(u32, HANDLE, Option<u32>)>,
}

impl Restore {
    /// 记下三个标准句柄和它们这一刻的继承标记。
    fn now() -> Restore {
        let saved = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE]
            .into_iter()
            .map(|which| {
                // SAFETY: 只读本进程的标准句柄表，参数是三个常量之一。
                let handle = unsafe { GetStdHandle(which) };
                let mut flags = 0;
                let usable = !handle.is_null() && handle != INVALID_HANDLE_VALUE;
                // SAFETY: `handle` 是本进程的标准句柄，判过不是空的、不是无效值；`flags` 是本函数里的变量。
                let read = usable && unsafe { GetHandleInformation(handle, &mut flags) } != 0;
                (which, handle, read.then_some(flags & HANDLE_FLAG_INHERIT))
            })
            .collect();
        Restore { saved }
    }
}

impl Drop for Restore {
    fn drop(&mut self) {
        for &(which, handle, flag) in &self.saved {
            // SAFETY: 放回的是记下时本进程的标准句柄（测试期间没关过它）；继承标记只放回记下时读到的那一位。
            unsafe {
                SetStdHandle(which, handle);
                if let Some(flag) = flag {
                    SetHandleInformation(handle, HANDLE_FLAG_INHERIT, flag);
                }
            }
        }
    }
}

/// 只这一条测试动标准句柄表：两条同时换、各自放回，会把对方换进去的句柄放回成标准输入（施工 FL-6）。
#[test]
fn a_redirected_standard_input_is_no_longer_inherited() {
    let before = Restore::now().saved.clone();
    let path = std::env::temp_dir().join(format!("miyu-stdio-{}", std::process::id()));
    std::fs::write(&path, b"prompt").expect("写得进");
    let file = File::open(&path).expect("开得了");
    let handle = file.as_raw_handle();
    // 后于 `file` 声明、先于它放下：断言失败时也是标准句柄先换回去、文件再关（施工 FL-6）。
    let restore = Restore::now();
    // SAFETY: `handle` 是上面开着的文件，`restore` 放回以前不关；只设继承标记、换本进程的标准输入（测试进程不读它）。
    unsafe {
        SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT);
        SetStdHandle(STD_INPUT_HANDLE, handle);
    }
    assert!(inheritable(handle), "重定向来的标准输入原本能继承");
    uninherit_std_handles();
    assert!(!inheritable(handle), "拉起子进程以前清掉了继承标记");
    drop(restore);
    drop(file);
    std::fs::remove_file(&path).expect("删得掉");
    // 走完以后标准句柄和继承标记都是原来的，同一个测试程序里后面再起子进程不受影响。
    assert_eq!(
        Restore::now().saved.clone(),
        before,
        "标准句柄和继承标记都放回去了"
    );
    let mut child = std::process::Command::new("cmd")
        .args(["/c", "exit 0"])
        .spawn()
        .expect("起得来");
    assert!(child.wait().expect("等得到").success());
}
