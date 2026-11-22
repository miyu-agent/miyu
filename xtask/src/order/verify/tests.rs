//! 核对排上了：排上了的、完全没排的、清单里有符号表里没有的名字、空清单；苹果的符号表（大小全是 0，施工 MO-1）上算出的
//! 比例和手算的一样。

use super::*;

/// 一台 M1 MacBook 上苹果的 `nm --defined-only --numeric-sort -S` 读一份没排过的核心的输出（2026-11-22）：开头、`_main`
/// 前后、sqlite 的 C 函数、代码段的末尾接着别的节，最后几行是数据段。几段之间删掉了中间的行。和
/// `crates/miyu-perf/src/order/listing/apple-nm.txt` 是同一份。
const APPLE_NM: &str = include_str!("apple-nm.txt");

/// 照 `nm --defined-only --numeric-sort -S` 的样子造一行。
fn line(address: u64, size: u64, kind: &str, name: &str) -> String {
    format!("{address:016x} {size:016x} {kind} {name}\n")
}

/// 一个假的程序：代码段从 0x1000 起，`hot_*` 四个各 0x100 字节，`cold_*` 二十个各 0x100 字节；`hot_first` 决定热的四个
/// 排在开头还是散在冷的中间。另带一个没有大小的符号、一个数据段的符号，核对时不该算进去。
fn program(hot_first: bool) -> String {
    let mut names: Vec<String> = (0..20).map(|index| format!("cold_{index}")).collect();
    for index in 0..4 {
        let hot = format!("hot_{index}");
        match hot_first {
            true => names.insert(index, hot),
            false => names.insert(index * 6 + 5, hot),
        }
    }
    let mut text = format!("{:016x} T _start_marker\n", 0x1000);
    for (index, name) in names.iter().enumerate() {
        text.push_str(&line(0x1000 + 0x100 * index as u64, 0x100, "t", name));
    }
    text.push_str(&line(0x9000, 0x40, "D", "hot_data"));
    text
}

const LIST: &str = "hot_0\nhot_1\nhot_2\nhot_3\n";

#[test]
fn listed_functions_at_the_front_pass() {
    let placed = check(&program(true), LIST).expect("排上了");
    assert_eq!(
        placed,
        Placed {
            listed: 4,
            found: 4,
            bytes: 0x400,
            near: 0x400
        }
    );
    assert!(placed.to_string().contains("100%"), "{placed}");
}

#[test]
fn scattered_functions_fail_with_the_share() {
    let error = check(&program(false), LIST).unwrap_err();
    assert!(error.contains("不到九成"), "{error}");
    // 开头 0x500 字节里只有 hot_0（排在第 5 个，起点 0x1500 之外的不算）：0%。
    assert!(error.contains("0% 排在代码段开头"), "{error}");
    assert!(error.contains("链接器"), "{error}");
}

#[test]
fn names_missing_from_the_symbol_table_are_skipped() {
    let list = format!("_start\n{LIST}not_in_this_program\nhot_data\n");
    let placed = check(&program(true), &list).expect("找不到的名字不算");
    assert_eq!(placed.listed, 7);
    assert_eq!(placed.found, 4);
    assert_eq!(placed.bytes, 0x400);
}

#[test]
fn an_empty_list_fails() {
    assert_eq!(check(&program(true), "").unwrap_err(), "清单是空的");
    assert_eq!(check(&program(true), "\n  \n").unwrap_err(), "清单是空的");
}

#[test]
fn a_list_with_nothing_found_fails() {
    let error = check(&program(true), "nothing\nhere\n").unwrap_err();
    assert!(error.contains("一个都找不到"), "{error}");
}

#[test]
fn just_under_nine_tenths_fails_and_just_at_passes() {
    // 清单：十个各 0x100 字节的函数。前九个挨着排在开头，第十个在很远的地方：正好九成，算过。
    let mut nm = String::new();
    for index in 0..9 {
        nm.push_str(&line(
            0x1000 + 0x100 * index,
            0x100,
            "t",
            &format!("f{index}"),
        ));
    }
    nm.push_str(&line(0x10_0000, 0x100, "t", "f9"));
    let list: String = (0..10).map(|index| format!("f{index}\n")).collect();
    assert!(check(&nm, &list).is_ok());
    // 再加一个远处的、大一点的，九成就不到了。
    nm.push_str(&line(0x20_0000, 0x200, "W", "f10"));
    let error = check(&nm, &format!("{list}f10\n")).unwrap_err();
    assert!(error.contains("75% 排在代码段开头"), "{error}");
}

#[test]
fn same_named_locals_count_together() {
    let nm = [
        line(0x1000, 0x100, "t", "dup"),
        line(0x1100, 0x100, "t", "dup"),
        line(0x1200, 0x100, "T", "other"),
    ]
    .concat();
    let placed = check(&nm, "dup\ndup\n").expect("同名的一起挪");
    assert_eq!((placed.listed, placed.found, placed.bytes), (1, 1, 0x200));
}

#[test]
fn names_trim_and_drop_repeats() {
    assert_eq!(names(" a \n\nb\na\n"), vec!["a", "b"]);
}

/// 照苹果的样子去掉大小：每一行带大小的那一列都写 0。
fn sizeless(nm: &str) -> String {
    nm.lines()
        .map(|row| {
            let fields: Vec<&str> = row.split(' ').collect();
            match fields.as_slice() {
                [address, _, kind, name] => format!("{address} {:016x} {kind} {name}\n", 0),
                _ => format!("{row}\n"),
            }
        })
        .collect()
}

#[test]
fn apple_symbols_without_sizes_check_like_linux() {
    // 每个函数照下一个符号的起点补成 0x100 字节。最后一个热的函数后面接着另一节的符号，它也是 0x100。
    let apple = |hot_first| {
        format!(
            "{}{:016x} {:016x} s GCC_except_table0\n",
            sizeless(&program(hot_first)),
            0x2800,
            0
        )
    };
    assert_eq!(check(&apple(true), LIST), check(&program(true), LIST));
    let error = check(&apple(false), LIST).unwrap_err();
    assert!(error.contains("0% 排在代码段开头"), "{error}");
}

#[test]
fn apple_shares_match_the_hand_count() {
    // f0 0x100、f1 0x200、g 0x400 字节（照下一个符号算），L = 0x700，开头的范围到 0x1000 + 0x700 + 0x1c0 = 0x18c0：
    // f0、f1 在里面，g 不在，0x300 / 0x700 是 42%。
    let nm = "\
0000000000001000 0000000000000000 t f0
0000000000001100 0000000000000000 t f1
0000000000001300 0000000000000000 t f2
0000000000008000 0000000000000000 t g
0000000000008400 0000000000000000 s GCC_except_table0
";
    let error = check(nm, "f0\nf1\ng\n").unwrap_err();
    assert!(
        error.contains("合计 1792 字节，42% 排在代码段开头"),
        "{error}"
    );
    // 最后一个函数后面没有符号：给它 LAST 字节。
    let placed =
        check("0000000000001000 0000000000000000 T only\n", "only\n").expect("一个函数总在开头");
    assert_eq!(placed.bytes, LAST);
}

#[test]
fn a_real_apple_table_checks_by_the_next_symbol() {
    // 真的苹果 `nm` 输出：文件头不算，代码段从第一个函数 0x100000ac0 起。
    let first = "__RINvMNtCskumHb0IaX0X_4core6optionINtB3_6OptionNtNtCs6SjEax68zxx_5alloc6string6StringE11map_or_elseNtNtCs9o5yL0itoBe_10serde_json5value5ValueNCNCNvNtCs3hjz7dYl0ZQ_8miyu_cli3pkg3act00NCB2g_s_0ECsbjKXPeU4cYC_4miyu";
    let second = "__RINvMNtCskumHb0IaX0X_4core6optionINtB3_6OptionReE3mapbNvYNtCsbjKXPeU4cYC_4miyu10CoreActionNtNtCs3ps2HsQ4hiK_12clap_builder6derive10Subcommand14has_subcommandEBV_";
    let third = "__RINvMsk_NtCsfiKLhsWjRsE_3std7processNtB6_7Command4envsINtNtCs6SjEax68zxx_5alloc3vec3VecTReNtNtNtB8_3ffi6os_str8OsStringEEB1o_B1q_ECsbjKXPeU4cYC_4miyu";
    // 开头的三个：0x1a0 + 0x80 + 0xd4，全在开头。
    let placed = check(APPLE_NM, &format!("{first}\n{second}\n{third}\n")).expect("开头的三个");
    assert_eq!((placed.found, placed.bytes, placed.near), (3, 0x2f4, 0x2f4));
    // 开头的两个加 `_main`（0x56c）：L = 0x78c，范围到 0x100000ac0 + 0x96f，`_main` 在 0x100076588，0x220 / 0x78c 是 28%。
    let error = check(APPLE_NM, &format!("{first}\n{second}\n_main\n")).unwrap_err();
    assert!(
        error.contains("合计 1932 字节，28% 排在代码段开头"),
        "{error}"
    );
    // 代码段的最后一个函数到另一节的 `GCC_except_table0`；数据段的符号不算函数。
    let last = "__RINvNtCskumHb0IaX0X_4core9panicking13assert_failedNtCsdBvu9k3p4Ny_7zlib_rs10ReturnCodeBM_EBO_";
    let error = check(APPLE_NM, &format!("{last}\n_sqlite3_temp_directory\n")).unwrap_err();
    assert!(error.contains("找到 1 个、合计 7240 字节"), "{error}");
}
