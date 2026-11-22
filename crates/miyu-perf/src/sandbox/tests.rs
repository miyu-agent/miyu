//! 装载起点照 `/proc/<进程号>/maps` 读（施工 LW-1）：路径对得上、偏移是 0 的那一段。

#[cfg(unix)]
use std::ffi::OsString;
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;

#[cfg(unix)]
use super::Probed;
use super::load_base;

/// 模拟器的映射表的样子：模拟器自己、被测的程序几段（头一段偏移 0）、动态库、匿名的、路径带空格的。
const MAPS: &str = "\
5612a0000000-5612a0100000 r-xp 00000000 00:1f 11 /usr/bin/qemu-x86_64
7f0000000000-7f0000400000 r--p 00000000 00:1f 22 /w/target/release/miyu
7f0000400000-7f0002000000 r-xp 00400000 00:1f 22 /w/target/release/miyu
7f0002000000-7f0002100000 rw-p 01fff000 00:1f 22 /w/target/release/miyu
7f1000000000-7f1000200000 r-xp 00000000 00:1f 33 /usr/lib/libc.so.6
7f2000000000-7f2000001000 rw-p 00000000 00:00 0 
7f3000000000-7f3000001000 r--p 00000000 00:1f 44 /w/with space/miyu
";

#[test]
fn the_base_is_the_mapping_at_offset_zero() {
    assert_eq!(
        load_base(MAPS, Path::new("/w/target/release/miyu")),
        Some(0x7f00_0000_0000)
    );
    assert_eq!(
        load_base(MAPS, Path::new("/w/with space/miyu")),
        Some(0x7f30_0000_0000)
    );
}

#[test]
fn other_programs_and_partial_paths_are_not_the_program() {
    assert_eq!(load_base(MAPS, Path::new("/w/target/release/mi")), None);
    assert_eq!(load_base(MAPS, Path::new("/elsewhere/miyu")), None);
    assert_eq!(load_base("", Path::new("/w/target/release/miyu")), None);
    // 只有偏移不是 0 的那几段的，不算装上了。
    let later = "7f0000400000-7f0002000000 r-xp 00400000 00:1f 22 /w/target/release/miyu\n";
    assert_eq!(load_base(later, Path::new("/w/target/release/miyu")), None);
}

/// 带页探针起（施工 MO-2）：四个环境变量，路径都是绝对的（被测进程的工作目录不是量尺的），重关的间隔是 20 毫秒（`REARM_MS`）。
/// 只在 Unix 上跑：探针只在 macOS 上用，这里照 Unix 的路径写法核对（Windows 上解开的路径带 `\\?\` 前缀、分隔符也不同）。
#[cfg(unix)]
#[test]
fn the_probe_goes_in_through_four_variables() {
    let dir = std::env::temp_dir().join(format!("miyu-perf-probed-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建得了");
    let dir = dir.canonicalize().expect("解得开");
    std::fs::write(dir.join("pagetrace.dylib"), "").expect("写得进");
    std::fs::write(dir.join("miyu"), "").expect("写得进");
    let probed = Probed {
        library: &dir.join("pagetrace.dylib"),
        out: Path::new("relative/order-1.pages"),
        program: &dir.join(".").join("miyu"),
    };
    let here = std::env::current_dir().expect("有工作目录");
    let os = |path: PathBuf| path.into_os_string();
    assert_eq!(
        probed.vars().expect("拼得出"),
        [
            ("DYLD_INSERT_LIBRARIES", os(dir.join("pagetrace.dylib"))),
            ("MIYU_PAGES_OUT", os(here.join("relative/order-1.pages"))),
            ("MIYU_PAGES_PROGRAM", os(dir.join("miyu"))),
            ("MIYU_PAGES_REARM_MS", OsString::from("20")),
        ]
    );
    let missing = Probed {
        library: &dir.join("missing.dylib"),
        ..probed
    };
    let error = missing.vars().unwrap_err();
    assert!(error.starts_with("找不到探针 "), "{error}");
    let missing = Probed {
        program: &dir.join("missing"),
        ..probed
    };
    assert!(missing.vars().unwrap_err().starts_with("找不到 "));
    std::fs::remove_dir_all(&dir).expect("删得掉");
}
