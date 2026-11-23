//! 清单的读法（施工 LW-1）：符号表只留代码段里的函数；日志只认指令行；地址减去装载起点、落在函数的范围里才算；先后照头一回
//! 执行，交错的线程块、同一个函数出现多次、两段日志接着读都照这一条。苹果的符号表（大小全是 0）和页探针的记录（施工
//! MO-1）：大小照下一个符号补，探针的文件正常、空、截断、头不对、满了、只有读的记录各是什么结果。

use std::io::Cursor;

use super::{LAST, Listing, Probe, Symbols, address};

/// 一台 M1 MacBook 上苹果的 `nm --defined-only --numeric-sort -S` 读一份没排过的核心的输出（2026-11-22）：开头、`_main`
/// 前后、sqlite 的 C 函数、代码段的末尾接着别的节，最后几行是数据段。几段之间删掉了中间的行。
const APPLE_NM: &str = include_str!("apple-nm.txt");

/// 造的符号表：几个函数（有一个弱的、一个和别人同名的局部函数、一个大小是 0 的），中间留着空隙；数据、没写大小的不算。
const NM: &str = "\
0000000000001000 0000000000000040 T main
0000000000001040 0000000000000020 t helper
0000000000001100 0000000000000080 W weak_one
0000000000001200 0000000000000010 t init
0000000000001300 0000000000000010 t init
0000000000001400 0000000000000000 T empty
0000000000002000 0000000000000100 D some_data
0000000000002100 0000000000000008 R some_rodata
0000000000003000 T no_size
";

/// 装载起点。
const BASE: u64 = 0x7f00_0000_0000;

/// 日志里的一行指令：地址是装载以后的。
fn at(address: u64) -> String {
    format!(
        "0x{:012x}:  48 89 e7                 movq     %rsp, %rdi\n",
        BASE + address
    )
}

/// 一块：分隔线、`IN:`，再是几行指令。
fn block(addresses: &[u64]) -> String {
    let mut text = String::from("----------------\nIN: \n");
    for &address in addresses {
        text.push_str(&at(address));
    }
    text.push('\n');
    text
}

#[test]
fn only_functions_in_the_code_count() {
    let symbols = Symbols::parse(NM);
    assert_eq!(symbols.count(), 6);
    assert_eq!(symbols.find(0x1000), Some(0));
    assert_eq!(symbols.find(0x103f), Some(0));
    assert_eq!(symbols.find(0x1040), Some(1));
    // 两个函数之间的空隙、头一个函数以前、数据里的都不算。
    assert_eq!(symbols.find(0x1060), None);
    assert_eq!(symbols.find(0x0fff), None);
    assert_eq!(symbols.find(0x2000), None);
    // 大小是 0 的当一个字节。
    assert_eq!(symbols.find(0x1400), Some(5));
    assert_eq!(symbols.find(0x1401), None);
}

#[test]
fn only_instruction_lines_have_an_address() {
    assert_eq!(
        address(b"0x7faae70eb7e0:  48 89 e7                 movq     %rsp, %rdi\n"),
        Some(0x7faa_e70e_b7e0)
    );
    assert_eq!(
        address(b"0x10:  c3                       retq     \n"),
        Some(0x10)
    );
    // 不带反汇编的 QEMU：一块只有一行地址，后面两个空格就到行尾，字节在下面的 `OBJD-T:` 行里。
    assert_eq!(address(b"0x2aaaab2cc1d0:  \n"), Some(0x2aaa_ab2c_c1d0));
    assert_eq!(address(b"0x10:  "), Some(0x10));
    for line in [
        &b"----------------\n"[..],
        b"IN: \n",
        b"IN: main\n",
        b"\n",
        b"0x:  48 89\n",
        b"0x10: 48 89 e7\n",
        b"0x10:  4\n",
        b"0x10:  zz 89\n",
        b"  0x10:  48 89\n",
        b"callq    0x7faae70ec490\n",
        b"OBJD-T: f30f1efa55488d0d24fefdff488d05f5\n",
        b"0x10:\n",
    ] {
        assert_eq!(address(line), None, "{}", String::from_utf8_lossy(line));
    }
}

#[test]
fn the_order_is_first_execution_across_threads_and_segments() {
    let symbols = Symbols::parse(NM);
    let mut listing = Listing::new(&symbols);
    // 两条线程的块交错：helper、main 交替，main 出现好几次；空隙里的、动态库的（装载起点以前、程序以外）、坏行不算。
    let first = [
        block(&[0x1040, 0x1044]),
        block(&[0x1000, 0x1004]),
        block(&[0x1060]),
        format!("0x{:012x}:  c3   retq\n", 0x5555_0000_0000_u64),
        format!("0x{:012x}:  c3   retq\n", BASE + 0x0100_0000),
        block(&[0x1048, 0x1008]),
        "0xzz garbage\n".to_string(),
        block(&[0x1200]),
    ]
    .concat();
    listing.read(BASE, Cursor::new(first)).unwrap();
    assert_eq!(listing.names(), ["helper", "main", "init"]);
    // 第二段装在别处：新出现的接在后面，出现过的不再记；同名的另一个局部函数不再写一次名字，大小照样算进去。
    let other = BASE + 0x10_0000;
    let second = [0x1000_u64, 0x1100, 0x1300, 0x1400]
        .map(|address| format!("0x{:012x}:  c3   retq\n", other + address))
        .concat();
    listing.read(other, Cursor::new(second)).unwrap();
    assert_eq!(
        listing.names(),
        ["helper", "main", "init", "weak_one", "empty"]
    );
    assert_eq!(listing.bytes(), 0x20 + 0x40 + 0x10 + 0x80 + 0x10);
}

#[test]
fn an_empty_log_lists_nothing() {
    let symbols = Symbols::parse(NM);
    let mut listing = Listing::new(&symbols);
    listing.read(BASE, Cursor::new("")).unwrap();
    assert!(listing.names().is_empty());
    assert_eq!(listing.bytes(), 0);
    assert_eq!(Symbols::parse("").count(), 0);
}

#[test]
fn apple_sizes_come_from_the_next_symbol() {
    let symbols = Symbols::parse(APPLE_NM);
    // 41 个代码段里的函数（`t`、`T`）；文件头 `__mh_execute_header` 也是 `T`，不算；`s`、`b`、`S` 不算。
    assert_eq!(symbols.count(), 41);
    assert_eq!(symbols.find(0x1_0000_0000), None);
    let size = |index: usize| symbols.functions[index].size;
    // 相邻的两个：0x100000ac0 到 0x100000c60。
    assert_eq!(symbols.functions[0].start, 0x1_0000_0ac0);
    assert_eq!(size(0), 0x1a0);
    assert_eq!(symbols.find(0x1_0000_0c5f), Some(0));
    assert_eq!(symbols.find(0x1_0000_0c60), Some(1));
    // `_main` 到下一个 Rust 函数，C 函数之间也一样（名字原样留着开头的下划线）。
    assert_eq!(symbols.functions[15].name, "_main");
    assert_eq!(size(15), 0x56c);
    assert_eq!(symbols.functions[20].name, "_sqlite3_result_text");
    assert_eq!(size(20), 0x28);
    // 代码段里最后一个函数到下一个符号（另一节的 `GCC_except_table0`）。
    assert_eq!(symbols.functions[40].start, 0x1_0147_ed2c);
    assert_eq!(size(40), 0x1_0148_0974 - 0x1_0147_ed2c);
    assert_eq!(symbols.find(0x1_0148_0973), Some(40));
    assert_eq!(symbols.find(0x1_0148_0974), None);
}

#[test]
fn the_last_apple_function_without_a_next_symbol_gets_a_page() {
    let nm = "\
0000000100000ac0 0000000000000000 t _first
0000000100000ac0 0000000000000000 t _alias
0000000100000b00 0000000000000000 T _last
";
    let symbols = Symbols::parse(nm);
    assert_eq!(symbols.functions[0].size, 0x40);
    assert_eq!(symbols.functions[1].size, 0x40, "起点相同的别名一样大");
    assert_eq!(symbols.functions[2].size, LAST);
    assert_eq!(symbols.find(0x1_0000_0b00 + LAST - 1), Some(2));
    assert_eq!(symbols.find(0x1_0000_0b00 + LAST), None);
}

#[test]
fn linux_zero_sizes_stay_one_byte() {
    // Linux 的表里有带大小的函数，大小是 0 的不补：`empty` 后面的数据不认成它。
    let symbols = Symbols::parse(NM);
    assert_eq!(symbols.find(0x1400), Some(5));
    assert_eq!(symbols.find(0x1401), None);
}

/// 照探针的格式造一份记录文件：上限 `capacity`、标志 `flags`，记录是（地址、种类）。
fn probe_file(capacity: u64, flags: u64, records: &[(u64, u64)]) -> Vec<u8> {
    let head = [
        u64::from_le_bytes(*b"MIYUPGT1"),
        0x1_0000_0000,
        0x4000,
        0x1_0000_0ac0,
        0x147_e294,
        capacity,
        records.len() as u64,
        flags,
    ];
    let words = records.iter().flat_map(|&(address, kind)| [address, kind]);
    head.into_iter()
        .chain(words)
        .flat_map(u64::to_le_bytes)
        .collect()
}

#[test]
fn a_probe_file_lists_executed_addresses_in_order() {
    let file = probe_file(
        4096,
        0,
        &[
            (0x1_0007_6600, 1),
            (0x1_0000_0ac8, 2),
            (0x1_0000_0c70, 1),
            (0x1_0147_ed30, 1),
        ],
    );
    let probe = Probe::parse(&file).expect("正常的记录");
    assert_eq!(
        probe,
        Probe {
            executed: vec![0x1_0007_6600, 0x1_0000_0c70, 0x1_0147_ed30],
            records: 4,
            full: false,
            page: 0x4000,
            // 读的那一条也算碰过；同一页的两条只算一页。
            pages: [0x1_0000_0000, 0x1_0007_4000, 0x1_0147_c000].into(),
            // `__text` 从 0x1_0000_0ac0 起、0x147_e294 字节：开头、末尾不满一页的两截不在里面。
            covered: (0x1_0000_4000, 0x1_0147_c000),
        }
    );
    // 交给同一个 `Listing`：地址不用减装载起点；函数中间的地址也认得（从别的页返回到这一页）。
    let symbols = Symbols::parse(APPLE_NM);
    let mut listing = Listing::new(&symbols);
    listing.add(&probe.executed);
    // 再来一份进程的记录：出现过的不再记，新的接在后面。
    listing.add(&[0x1_0000_0c70, 0x1_0136_cfb8]);
    let names = listing.names();
    assert_eq!(names.len(), 4);
    assert_eq!(names[0], "_main");
    assert!(names[1].starts_with("__RINvMNtCskumHb0IaX0X_4core6optionINtB3_6OptionReE3map"));
    assert!(
        names[2].starts_with(
            "__RINvNtCskumHb0IaX0X_4core9panicking13assert_failedNtCsdBvu9k3p4Ny_7zlib"
        )
    );
    assert_eq!(names[3], "_sqlite3_result_text");
    assert_eq!(listing.bytes(), 0x56c + 0x80 + 0x1c48 + 0x28);
}

#[test]
fn an_empty_probe_file_and_one_without_records() {
    assert!(Probe::parse(&[]).unwrap_err().contains("空文件"));
    let probe = Probe::parse(&probe_file(4096, 0, &[])).expect("没有记录也是一份正常的记录");
    assert_eq!(
        (probe.executed.len(), probe.records, probe.full),
        (0, 0, false)
    );
}

#[test]
fn a_truncated_probe_file_fails() {
    let file = probe_file(4096, 0, &[(0x1_0000_0c70, 1), (0x1_0000_0d00, 1)]);
    // 文件头不全。
    let error = Probe::parse(&file[..40]).unwrap_err();
    assert!(
        error.contains("截断") && error.contains("40 字节"),
        "{error}"
    );
    // 少了半条记录。
    let error = Probe::parse(&file[..file.len() - 8]).unwrap_err();
    assert!(error.contains("截断") && error.contains("2 条"), "{error}");
    // 多出字节。
    let mut longer = file.clone();
    longer.push(0);
    assert!(Probe::parse(&longer).unwrap_err().contains("多出"));
}

#[test]
fn a_probe_file_with_a_bad_header_fails() {
    let mut file = probe_file(4096, 0, &[(0x1_0000_0c70, 1)]);
    file[0] = b'X';
    assert!(Probe::parse(&file).unwrap_err().contains("魔数"));
    let error = Probe::parse(&probe_file(4096, 2, &[])).unwrap_err();
    assert!(error.contains("标志"), "{error}");
    let error = Probe::parse(&probe_file(1, 0, &[(1, 1), (2, 1)])).unwrap_err();
    assert!(error.contains("超过了上限"), "{error}");
    let error = Probe::parse(&probe_file(4096, 0, &[(1, 1), (2, 3)])).unwrap_err();
    assert!(error.contains("第 2 条") && error.contains("3"), "{error}");
    let error = Probe::parse(&probe_file(4096, 0, &[(1, 0)])).unwrap_err();
    assert!(error.contains("种类是 0"), "{error}");
}

#[test]
fn a_full_probe_file_still_reads() {
    let probe = Probe::parse(&probe_file(2, 1, &[(0x1_0000_0c70, 1), (0x1_0000_0ac0, 1)]))
        .expect("满了的照样读");
    assert!(probe.full);
    assert_eq!(probe.executed, [0x1_0000_0c70, 0x1_0000_0ac0]);
}

#[test]
fn a_probe_file_with_only_reads_lists_nothing() {
    let probe = Probe::parse(&probe_file(
        4096,
        0,
        &[(0x1_0000_0c70, 2), (0x1_0000_0ac0, 2)],
    ))
    .expect("只有读的记录");
    assert_eq!((probe.executed.len(), probe.records), (0, 2));
    let symbols = Symbols::parse(APPLE_NM);
    let mut listing = Listing::new(&symbols);
    listing.add(&probe.executed);
    assert!(listing.names().is_empty());
}
