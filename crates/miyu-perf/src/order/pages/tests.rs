//! 读一段的页探针记录（施工 MO-2）：照 MO-1 的格式手工造几份假的记录和旁注，看挑哪几份、照什么先后、写出的清单、记满了
//! 和一份都没有时怎么办、删的时候删哪些。

use std::path::{Path, PathBuf};

#[cfg(unix)]
use super::names;
use super::{Read, pid_of, read, remove};
use crate::order::listing::{Listing, Symbols};

/// 造的符号表（苹果的样子，大小全是 0，照下一个符号补）：四个函数，后面接着数据。
const NM: &str = "\
0000000100000000 0000000000000000 T __mh_execute_header
0000000100001000 0000000000000000 T _main
0000000100001100 0000000000000000 t _helper
0000000100001200 0000000000000000 T _sqlite3_open
0000000100001300 0000000000000000 t _late
0000000100002000 0000000000000000 D _some_data
";

/// 照探针的格式造一份记录文件：标志 `flags`，记录是（地址、种类）。
fn probe_file(flags: u64, records: &[(u64, u64)]) -> Vec<u8> {
    let head = [
        u64::from_le_bytes(*b"MIYUPGT1"),
        0x1_0000_0000,
        0x4000,
        0x1_0000_1000,
        0x1000,
        4096,
        records.len() as u64,
        flags,
    ];
    let words = records.iter().flat_map(|&(address, kind)| [address, kind]);
    head.into_iter()
        .chain(words)
        .flat_map(u64::to_le_bytes)
        .collect()
}

/// 一个新的空目录，里面放着假的被测程序 `miyu` 和别的程序 `miyu-sandbox`（路径带一个空格：旁注里的路径可以有空格）。
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "miyu-perf-pages-{name}-{} with space",
        std::process::id()
    ));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("删得掉");
    }
    std::fs::create_dir_all(&dir).expect("建得了");
    let dir = dir.canonicalize().expect("解得开");
    std::fs::write(dir.join("miyu"), "").expect("写得进");
    std::fs::write(dir.join("miyu-sandbox"), "").expect("写得进");
    dir
}

/// 写一份进程的记录和旁注：前缀 `prefix`、进程号 `pid`，旁注写 `note`。
fn process(prefix: &Path, pid: u32, note: &str, records: &[u8]) {
    let file = format!("{}.{pid}", prefix.display());
    std::fs::write(&file, records).expect("写得进");
    std::fs::write(format!("{file}.exe"), note).expect("写得进");
}

#[test]
fn only_numbered_files_of_this_prefix_are_records() {
    assert_eq!(pid_of("order-1.pages.4242", "order-1.pages"), Some(4242));
    for other in [
        "order-1.pages.4242.exe",
        "order-1.pages.4242.tmp",
        "order-1.pages.4242.exe.tmp",
        "order-1.pages.",
        "order-1.pages",
        "order-1.pages.x1",
        "order-1.pages.+1",
        "order-1.pages.99999999999",
        "order-2.pages.4242",
        "order-1.pagesx.4242",
    ] {
        assert_eq!(pid_of(other, "order-1.pages"), None, "{other}");
    }
}

/// 只在 Unix 上跑：探针只在 macOS 上用，旁注里写的是 Unix 的路径（`/`、`/./`），Windows 上路径的写法对不上。
#[cfg(unix)]
#[test]
fn the_note_names_the_program_even_with_spaces_and_arguments() {
    let dir = scratch("names");
    let wanted = dir.join("miyu");
    let exe = wanted.display();
    assert!(names(&format!("{exe}\n"), &wanted));
    assert!(names(&format!("{exe} core --idle-seconds\n"), &wanted));
    // 没有换行、路径没解开（多一个 `.`）的也认。
    assert!(names(&format!("{}/./miyu core", dir.display()), &wanted));
    let sandbox = dir.join("miyu-sandbox");
    assert!(!names(&format!("{} run x\n", sandbox.display()), &wanted));
    assert!(!names("", &wanted));
    assert!(!names("/elsewhere/miyu core\n", &wanted));
    std::fs::remove_dir_all(&dir).expect("删得掉");
}

/// 一段里有被测程序的两个进程、别的程序的一个、没改完名的一份、别的段的一份：只读被测程序的两份，照进程号的先后，
/// 每份照记录的先后；读的记录不进清单；记满了的照样读。删的时候只删这一段的。
#[test]
fn a_segment_reads_the_program_files_in_order() {
    let dir = scratch("segment");
    let prefix = dir.join("order-3.pages");
    let miyu = dir.join("miyu");
    let core = format!("{} core --idle-seconds\n", miyu.display());
    // 进程号大的先写，读的时候照进程号排。
    process(
        &prefix,
        700,
        &core,
        &probe_file(0, &[(0x1_0000_1310, 1), (0x1_0000_1000, 1)]),
    );
    process(
        &prefix,
        90,
        &core,
        &probe_file(
            1,
            &[(0x1_0000_1204, 1), (0x1_0000_1100, 2), (0x1_0000_1000, 1)],
        ),
    );
    let sandbox = format!("{} run\n", dir.join("miyu-sandbox").display());
    process(&prefix, 95, &sandbox, &probe_file(0, &[(0x1_0000_1100, 1)]));
    std::fs::write(format!("{}.700.tmp", prefix.display()), "半份").expect("写得进");
    let other = dir.join("order-4.pages");
    process(&other, 5, &core, &probe_file(0, &[(0x1_0000_1100, 1)]));

    let symbols = Symbols::parse(NM);
    let mut listing = Listing::new(&symbols);
    let got = read(&prefix, &miyu, &mut listing).expect("读得了");
    assert_eq!(
        got,
        Read {
            files: 2,
            records: 5,
            executed: 4,
        }
    );
    // `_helper` 只被读过、只被别的程序执行过，不进清单。
    assert_eq!(listing.names(), ["_sqlite3_open", "_main", "_late"]);
    // 写出的清单和模拟器那一路同一个格式：符号名一行一个（`order.rs` 的 `write`）。
    assert_eq!(
        listing.names().join("\n") + "\n",
        "_sqlite3_open\n_main\n_late\n"
    );

    remove(&prefix).expect("删得掉");
    let mut left: Vec<String> = std::fs::read_dir(&dir)
        .expect("读得了")
        .map(|entry| {
            entry
                .expect("读得了")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    left.sort();
    assert_eq!(
        left,
        [
            "miyu",
            "miyu-sandbox",
            "order-4.pages.5",
            "order-4.pages.5.exe"
        ]
    );
    std::fs::remove_dir_all(&dir).expect("删得掉");
}

/// 一份被测程序的记录都没有、只有「读」的记录、记录坏了：报错停下，报错里写可能的原因。
#[test]
fn nothing_recorded_is_an_error_with_the_likely_reason() {
    let dir = scratch("empty");
    let prefix = dir.join("order-1.pages");
    let miyu = dir.join("miyu");
    let symbols = Symbols::parse(NM);

    let error = read(&prefix, &miyu, &mut Listing::new(&symbols)).unwrap_err();
    assert!(error.starts_with("探针没有写下 "), "{error}");
    assert!(error.contains("别的进程的 0 份不算"), "{error}");
    assert!(error.contains("加固运行时"), "{error}");

    // 只有别的程序的、只有记录没有旁注的。
    let sandbox = format!("{} run\n", dir.join("miyu-sandbox").display());
    process(&prefix, 10, &sandbox, &probe_file(0, &[(0x1_0000_1000, 1)]));
    std::fs::write(
        format!("{}.11", prefix.display()),
        probe_file(0, &[(0x1_0000_1000, 1)]),
    )
    .expect("写得进");
    let error = read(&prefix, &miyu, &mut Listing::new(&symbols)).unwrap_err();
    assert!(error.contains("别的进程的 2 份不算"), "{error}");

    let core = format!("{} core\n", miyu.display());
    process(&prefix, 12, &core, &probe_file(0, &[(0x1_0000_1000, 2)]));
    let error = read(&prefix, &miyu, &mut Listing::new(&symbols)).unwrap_err();
    assert!(error.contains("「执行」的记录一条都没有"), "{error}");
    assert!(error.contains("系统不让插库"), "{error}");

    process(&prefix, 13, &core, b"MIYUPGT1");
    let error = read(&prefix, &miyu, &mut Listing::new(&symbols)).unwrap_err();
    assert!(error.contains("order-1.pages.13："), "{error}");
    assert!(error.contains("截断"), "{error}");

    let error = read(&prefix, &dir.join("missing"), &mut Listing::new(&symbols)).unwrap_err();
    assert!(error.starts_with("找不到 "), "{error}");
    std::fs::remove_dir_all(&dir).expect("删得掉");
}
