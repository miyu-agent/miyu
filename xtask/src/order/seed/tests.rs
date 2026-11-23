//! 名字和符号互换：真的 Mach-O 符号（一台 M1 MacBook 上编的核心里抄来的，2026-11-23）换成起点清单的写法；`nm` 的输出
//! 里挑代码段的符号；名字换回符号时对不上的跳过、一个名字对上几个的都排。

use super::*;

#[test]
fn c_functions_lose_the_one_leading_underscore() {
    assert_eq!(name_of("_sqlite3_result_text"), "sqlite3_result_text");
    assert_eq!(name_of("_main"), "main");
    // 汇编里直接写的名字（ring 的）没有下划线，原样就是名字。
    assert_eq!(name_of("ChaCha20_512_neon"), "ChaCha20_512_neon");
    // 只去一个：C 里本来就以下划线开头的名字留着它自己的那一个。
    assert_eq!(name_of("__mh_execute_header"), "_mh_execute_header");
}

#[test]
fn v0_rust_symbols_demangle_without_the_crate_hash() {
    assert_eq!(
        name_of("__RNvMNtCs8zLmd3GcPz2_10miyu_store9knowledgeNtB2_6Reason4code"),
        "<miyu_store::knowledge::Reason>::code"
    );
    assert_eq!(
        name_of(
            "__RNvXsd_NtNtCsfiKLhsWjRsE_3std2io5stdioNtB5_6StdoutNtB7_5Write17is_write_vectored"
        ),
        "<std::io::stdio::Stdout as std::io::Write>::is_write_vectored"
    );
}

#[test]
fn legacy_rust_symbols_demangle_without_the_hash() {
    assert_eq!(
        name_of("__ZN4core3fmt5write17h3a4c5e6f7a8b9c0dE"),
        "core::fmt::write"
    );
}

#[test]
fn compiler_suffixes_are_dropped_before_demangling() {
    assert_eq!(
        name_of("__RNvMNtCs4Emt5dNfp2Q_4jiff9timestampNtB2_9Timestamp8to_zoned.2848"),
        "<jiff::timestamp::Timestamp>::to_zoned"
    );
    assert_eq!(
        name_of("__RNvMNtCskumHb0IaX0X_4core3stre20eq_ignore_ascii_case.llvm.1234567"),
        "<str>::eq_ignore_ascii_case"
    );
    assert_eq!(
        name_of("_sqlite3VdbeMemCopy.llvm.42.7"),
        "sqlite3VdbeMemCopy"
    );
    // 不是数字的后缀是另一段代码，不去。
    assert_eq!(name_of("_foo.cold"), "foo.cold");
    assert_eq!(name_of("_foo."), "foo.");
}

#[test]
fn a_symbol_that_only_looks_like_rust_stays_as_it_is() {
    assert_eq!(name_of("__Rnot_mangled"), "_Rnot_mangled");
}

#[test]
fn only_code_symbols_are_taken_from_nm() {
    let nm = "\
/t/deps/miyu-1269a927180401de.miyu.83d5e61c9971cbc0-cgu.0.rcgu.o:
0000000000000000 T _main
0000000000000040 t __RNvMNtCs8zLmd3GcPz2_10miyu_store9knowledgeNtB2_6Reason4code
0000000000001000 s GCC_except_table0
0000000000002000 D _data

/t/deps/rustcypPgvm/liblibsqlite3_sys-e22384acba2d89c8.rlib(sqlite3.o):
0000000000000100 T _sqlite3_result_text
0000000000000000 t ltmp0
";
    assert_eq!(
        code_symbols(nm),
        [
            "_main",
            "__RNvMNtCs8zLmd3GcPz2_10miyu_store9knowledgeNtB2_6Reason4code",
            "_sqlite3_result_text",
            "ltmp0",
        ]
    );
}

#[test]
fn names_map_to_every_matching_symbol_in_table_order() {
    let symbols = [
        "_main",
        "__RNvMNtCs4Emt5dNfp2Q_4jiff9timestampNtB2_9Timestamp8to_zoned.2848",
        "_sqlite3_result_text",
        "__RNvMNtCs4Emt5dNfp2Q_4jiff9timestampNtB2_9Timestamp8to_zoned.17",
        // 两个归档里都列了同一个符号：只写一次。
        "_sqlite3_result_text",
    ];
    let names = "sqlite3_result_text\n<jiff::timestamp::Timestamp>::to_zoned\ngone::function\n\nmain\nmain\n";
    let mapped = map(names, &symbols);
    assert_eq!(
        mapped,
        Mapped {
            listed: 4,
            found: 3,
            symbols: vec![
                "_sqlite3_result_text",
                "__RNvMNtCs4Emt5dNfp2Q_4jiff9timestampNtB2_9Timestamp8to_zoned.2848",
                "__RNvMNtCs4Emt5dNfp2Q_4jiff9timestampNtB2_9Timestamp8to_zoned.17",
                "_main",
            ],
        }
    );
}

#[test]
fn an_empty_list_maps_to_nothing() {
    let mapped = map("\n \n", &["_main"]);
    assert_eq!((mapped.listed, mapped.found), (0, 0));
    assert!(mapped.symbols.is_empty());
}

#[test]
fn a_list_is_one_line_each_with_a_final_newline() {
    assert_eq!(render(&["_main", "_foo"]), "_main\n_foo\n");
    assert_eq!(render(&[]), "");
}

#[test]
fn the_seed_lists_in_the_repository_are_names_not_symbols() {
    for (file, text) in [
        (
            "macos-miyu.txt",
            include_str!("../../../order/macos-miyu.txt"),
        ),
        (
            "macos-miyu-tui.txt",
            include_str!("../../../order/macos-miyu-tui.txt"),
        ),
    ] {
        let names = crate::order::verify::names(text);
        assert!(names.len() > 1000, "{file} 只有 {} 个名字", names.len());
        assert_eq!(names.first(), Some(&"main"), "{file}");
        // 写的是解开以后的名字：没有 Mach-O 的 Rust 符号，名字换一遍还是它自己。
        for name in names.iter().take(200) {
            assert!(!name.starts_with("__R"), "{file}：{name}");
        }
    }
}
