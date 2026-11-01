use std::fs::File;
use std::os::windows::io::AsRawHandle;

use windows_sys::Win32::Foundation::{
    GetHandleInformation, HANDLE_FLAG_INHERIT, SetHandleInformation,
};
use windows_sys::Win32::System::Console::{STD_INPUT_HANDLE, SetStdHandle};

use super::*;

/// 句柄 `handle` 的继承标记。
fn inheritable(handle: windows_sys::Win32::Foundation::HANDLE) -> bool {
    let mut flags = 0;
    // SAFETY: `handle` 是测试自己开着的文件的句柄，`flags` 是本函数里的变量。
    let read = unsafe { GetHandleInformation(handle, &mut flags) };
    assert_ne!(read, 0, "读得到句柄的标记");
    flags & HANDLE_FLAG_INHERIT != 0
}

#[test]
fn a_redirected_standard_input_is_no_longer_inherited() {
    let path = std::env::temp_dir().join(format!("miyu-stdio-{}", std::process::id()));
    std::fs::write(&path, b"prompt").expect("写得进");
    let file = File::open(&path).expect("开得了");
    let handle = file.as_raw_handle();
    // SAFETY: `handle` 是上面开着的文件，测试结束前不关；只设继承标记、换本进程的标准输入（测试进程不读它）。
    unsafe {
        SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT);
        SetStdHandle(STD_INPUT_HANDLE, handle);
    }
    assert!(inheritable(handle), "重定向来的标准输入原本能继承");
    uninherit_std_handles();
    assert!(!inheritable(handle), "拉起子进程以前清掉了继承标记");
    drop(file);
    std::fs::remove_file(&path).expect("删得掉");
}
