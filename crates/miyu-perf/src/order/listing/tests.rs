//! 清单的读法（施工 LW-1）：符号表只留代码段里的函数；日志只认指令行；地址减去装载起点、落在函数的范围里才算；先后照头一回
//! 执行，交错的线程块、同一个函数出现多次、两段日志接着读都照这一条。

use std::io::Cursor;

use super::{Listing, Symbols, address};

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
