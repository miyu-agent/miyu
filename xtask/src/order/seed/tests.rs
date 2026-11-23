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

/// 一份假程序：页大小 0x4000，起点清单里的三个函数排在最前面两页，后面是别的函数。
fn program() -> Vec<(&'static str, u64, u64)> {
    vec![
        ("_main", 0x1_0000_0000, 0x100),
        ("_sqlite3_result_text", 0x1_0000_0100, 0x3f00),
        // 跨了两页：后一页被碰过就算。
        (
            "__RNvMNtCs8zLmd3GcPz2_10miyu_store9knowledgeNtB2_6Reason4code",
            0x1_0000_3f00,
            0x200,
        ),
        ("_cold_one", 0x1_0000_8000, 0x100),
        ("_cold_two.17", 0x1_0000_c000, 0x100),
        ("_fresh", 0x1_0001_0000, 0x100),
    ]
}

fn touched(starts: &[u64]) -> Touched {
    Touched {
        page: 0x4000,
        covered: (0x1_0000_0000, 0x1_0002_0000),
        starts: starts.iter().copied().collect(),
    }
}

#[test]
fn a_merge_keeps_the_seed_order_appends_new_names_and_drops_untouched_pages() {
    let seed = "main\n<miyu_store::knowledge::Reason>::code\ncold_one\ncold_two\ngone::function\n";
    // 这一轮：碰过第 1 页和 _fresh 那一页（第 0 页没记到）；看到了 _fresh（新的）、_main（清单里有的、它那一页没记到也
    // 留着）、_fresh 的另一份。跨第 0、1 页的那个函数照第 1 页留着。
    let merged = merge(
        seed,
        &["_fresh", "_main", "_fresh"],
        &program(),
        &touched(&[0x1_0000_4000, 0x1_0001_0000]),
    );
    assert_eq!(
        merged,
        Merged {
            names: vec![
                "main".to_string(),
                "<miyu_store::knowledge::Reason>::code".to_string(),
                // 对不上符号的照原样留着。
                "gone::function".to_string(),
                "fresh".to_string(),
            ],
            added: 1,
            removed: 2,
            unmatched: 1,
        }
    );
}

#[test]
fn a_name_seen_this_round_is_kept_even_without_its_page() {
    // 页的记录里少了它那一页（例如记满了），可是清单里看到了它：留着。
    let merged = merge(
        "cold_one\n",
        &["_cold_one"],
        &program(),
        &touched(&[0x1_0000_0000]),
    );
    assert_eq!(
        (merged.names.len(), merged.removed, merged.added),
        (1, 0, 0)
    );
}

#[test]
fn touched_pages_parse_the_probe_file() {
    assert_eq!(
        Touched::parse("page 16384\ncovered 100000000 100020000\n100000000\n100004000\n").unwrap(),
        touched(&[0x1_0000_0000, 0x1_0000_4000])
    );
    for bad in [
        "",
        "page 0\ncovered 0 4000\n1000\n",
        "page x\n",
        "16384\n1000\n",
        "page 16384\n1000\n",
        "page 16384\ncovered 0 zz\n1000\n",
        "page 16384\ncovered 0 4000\nzz\n",
        "page 16384\ncovered 0 4000\n",
    ] {
        assert!(Touched::parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn a_name_list_with_problems_is_refused() {
    assert!(problems("main\nmiyu::main\n").is_empty());
    assert_eq!(problems(""), ["是空的"]);
    assert_eq!(problems("main"), ["最后一行没有换行"]);
    let found = problems("main\n\n main\n__RNvCs_4miyu4main\nmain\n");
    assert_eq!(found.len(), 4, "{found:?}");
    assert!(found[0].contains("第 2 行是空的"));
    assert!(found[1].contains("前后带空白"));
    assert!(found[2].contains("是符号不是名字"));
    assert!(found[3].contains("重复"));
}

#[test]
fn the_seed_lists_in_the_repository_have_no_problems() {
    for text in [
        include_str!("../../../order/macos-miyu.txt"),
        include_str!("../../../order/macos-miyu-tui.txt"),
    ] {
        assert_eq!(problems(text), Vec::<String>::new());
    }
}

/// `main` 和启动时执行的函数排在 `__text` 开头不满一页的那一截里，探针看不见它们（2026-11-23 第一轮把它们去掉了）：
/// 落在关上的那一段以外的不去，末尾那一截也一样。
#[test]
fn names_outside_the_probed_range_are_kept() {
    let functions = [
        ("_main", 0x1_0000_0ac0, 0x100),
        ("_in_range", 0x1_0000_8000, 0x100),
        ("_tail", 0x1_0001_ff00, 0x200),
    ];
    let touched = Touched {
        page: 0x4000,
        covered: (0x1_0000_4000, 0x1_0002_0000),
        starts: [0x1_0000_4000].into(),
    };
    let merged = merge("main\nin_range\ntail\n", &[], &functions, &touched);
    assert_eq!(merged.names, ["main", "tail"]);
    assert_eq!(merged.removed, 1);
}
