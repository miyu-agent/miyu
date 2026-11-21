//! 核对排上了：排上了的、完全没排的、清单里有符号表里没有的名字、空清单。

use super::*;

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
