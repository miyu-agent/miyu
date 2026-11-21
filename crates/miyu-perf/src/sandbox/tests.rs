//! 装载起点照 `/proc/<进程号>/maps` 读（施工 LW-1）：路径对得上、偏移是 0 的那一段。

use std::path::Path;

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
