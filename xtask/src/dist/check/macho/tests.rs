//! 读加载命令、认链式修正和最低系统：照 Mach-O 格式现搭的小文件测；macOS 上再读这个测试程序自己，证明读得懂真的链接器
//! 写出来的程序。

use super::*;

/// `LC_DYLD_INFO_ONLY`：老式修正。
const INFO_ONLY: u32 = 0x8000_0022;

/// 一个 64 位 arm64 的可执行文件，只有文件头和加载命令 `commands`（种类、命令里种类和长度后面的内容）。
fn macho(commands: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let put = |file: &mut Vec<u8>, value: u32| file.extend_from_slice(&value.to_le_bytes());
    let mut body = Vec::new();
    for (kind, rest) in commands {
        put(&mut body, *kind);
        put(&mut body, u32::try_from(8 + rest.len()).unwrap());
        body.extend_from_slice(rest);
    }
    let mut file = Vec::new();
    // 魔数、CPU（arm64）、子类型、文件类型（可执行）、命令条数、命令合计长度、标志、保留。
    for value in [
        MAGIC,
        0x0100_000c,
        0,
        2,
        u32::try_from(commands.len()).unwrap(),
        u32::try_from(body.len()).unwrap(),
        0,
        0,
    ] {
        put(&mut file, value);
    }
    file.extend(body);
    file
}

/// `LC_BUILD_VERSION`：平台是 macOS，最低系统 `minimum`，SDK 26.5，不带工具表。
fn build_version(minimum: &str) -> (u32, Vec<u8>) {
    let mut rest = Vec::new();
    for value in [1, packed(minimum).unwrap(), packed("26.5").unwrap(), 0] {
        rest.extend_from_slice(&value.to_le_bytes());
    }
    (BUILD_VERSION, rest)
}

/// 修正信息那一条：只看种类，内容是偏移和长度，这里写零。
fn fixups(kind: u32) -> (u32, Vec<u8>) {
    let size = if kind == CHAINED { 8 } else { 40 };
    (kind, vec![0; size])
}

/// 照最低系统 12.0 链接的样子（链式修正）。
fn chained_12() -> Vec<u8> {
    macho(&[(0x19, vec![0; 64]), fixups(CHAINED), build_version("12.0")])
}

/// 照默认的 11.0 链接的样子（rc.5 的两个程序都是这样）。
fn legacy_11() -> Vec<u8> {
    macho(&[
        (0x19, vec![0; 64]),
        fixups(INFO_ONLY),
        build_version("11.0"),
    ])
}

/// 测试的临时目录，用完删掉。
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("xtask-macho-{name}-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("临时目录没删掉：{error}");
        }
    }
}

#[test]
fn versions_are_written_the_mach_o_way() {
    assert_eq!(packed(MACOS_MINIMUM), Some(0x000c_0000));
    assert_eq!(packed("11.0"), Some(0x000b_0000));
    assert_eq!(packed("12.2.1"), Some(0x000c_0201));
    assert_eq!(packed("13"), Some(0x000d_0000));
    for bad in ["", "12.", "a.0", "12.256", "1.2.3.4", "65536.0"] {
        assert_eq!(packed(bad), None, "{bad}");
    }
    assert_eq!(version(0x000b_0000), "11.0");
    assert_eq!(version(0x000c_0201), "12.2.1");
}

#[test]
fn the_commands_say_which_fixups_and_minimum() {
    assert_eq!(
        commands(&chained_12()).unwrap(),
        Loaded {
            chained: true,
            legacy: false,
            minimum: packed("12.0"),
        }
    );
    assert_eq!(
        commands(&legacy_11()).unwrap(),
        Loaded {
            chained: false,
            legacy: true,
            minimum: packed("11.0"),
        }
    );
    // 老一代的 LC_VERSION_MIN_MACOSX、不带种类以外标志的 LC_DYLD_INFO 也认。
    let mut old = Vec::new();
    old.extend_from_slice(&packed("10.15").unwrap().to_le_bytes());
    old.extend_from_slice(&packed("10.15").unwrap().to_le_bytes());
    assert_eq!(
        commands(&macho(&[(VERSION_MIN_MACOSX, old), fixups(0x22)])).unwrap(),
        Loaded {
            chained: false,
            legacy: true,
            minimum: packed("10.15"),
        }
    );
    assert_eq!(commands(&macho(&[])).unwrap().minimum, None);
}

#[test]
fn what_is_not_a_whole_mach_o_file_is_an_error() {
    assert!(commands(b"").is_err());
    assert!(commands(b"#!/bin/sh\necho hi\n").is_err());
    // ELF、32 位的 Mach-O 都不认。
    assert!(commands(b"\x7fELF\x02\x01\x01\0\0\0\0\0\0\0\0\0").is_err());
    let mut thin = chained_12();
    thin[..4].copy_from_slice(&0xfeed_faceu32.to_le_bytes());
    assert!(commands(&thin).is_err());
    let mut fat = chained_12();
    fat[..4].copy_from_slice(&[0xca, 0xfe, 0xba, 0xbe]);
    assert!(commands(&fat).unwrap_err().contains("几种架构"));
    // 最后一条命令写到一半就截断了。
    let whole = chained_12();
    assert!(commands(&whole[..whole.len() - 3]).is_err());
    // 命令长度写成 0：不能原地打转。
    let mut stuck = chained_12();
    stuck[HEADER + 4..HEADER + 8].copy_from_slice(&0u32.to_le_bytes());
    assert!(commands(&stuck).unwrap_err().contains("长度"));
    // 命令条数比实际的多。
    let mut more = chained_12();
    more[16..20].copy_from_slice(&9u32.to_le_bytes());
    assert!(commands(&more).is_err());
}

#[test]
fn only_chained_fixups_at_the_minimum_pass() {
    let scratch = Scratch::new("problem");
    let file = scratch.0.join("miyu");
    let said = |bytes: Vec<u8>| {
        fs::write(&file, bytes).unwrap();
        problem(&file, "miyu")
    };
    assert_eq!(said(chained_12()), None);
    let old = said(legacy_11()).unwrap();
    assert!(
        old.starts_with(
            "miyu 没有 LC_DYLD_CHAINED_FIXUPS（链式修正）；有 LC_DYLD_INFO_ONLY（老式修正）；最低系统是 11.0，该是 12.0："
        ),
        "{old}"
    );
    assert!(old.contains("MACOSX_DEPLOYMENT_TARGET=12.0"), "{old}");
    // 11.0 加 -fixup_chains 强制用链式修正：程序写着支持 11，macOS 11 上不一定起得来，也不算。
    let forced = said(macho(&[fixups(CHAINED), build_version("11.0")])).unwrap();
    assert!(forced.contains("最低系统是 11.0，该是 12.0"), "{forced}");
    assert!(!forced.contains("链式修正"), "{forced}");
    // 比要的高也不算：写 13.0 会多排除 macOS 12 的人。
    let higher = said(macho(&[fixups(CHAINED), build_version("13.0")])).unwrap();
    assert!(higher.contains("最低系统是 13.0"), "{higher}");
    let unwritten = said(macho(&[fixups(CHAINED)])).unwrap();
    assert!(unwritten.contains("最低系统是 没写"), "{unwritten}");
    let text = said(b"program".to_vec()).unwrap();
    assert!(text.contains("加载命令读不了"), "{text}");
    fs::remove_file(&file).unwrap();
    assert!(problem(&file, "miyu").unwrap().contains("读不了"));
}

/// 打包前、查包时都照它查每个 macOS 程序；别的系统的包不查。
#[test]
fn the_macos_package_checks_every_program() {
    let scratch = Scratch::new("package");
    let repo = scratch.0.join("repo");
    let dir = scratch.0.join("pkg");
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir_all(&dir).unwrap();
    fs::write(repo.join("THIRD-PARTY-LICENSES.txt"), "core notices\n").unwrap();
    let programs: Vec<&str> = super::super::CORE
        .iter()
        .copied()
        .chain([super::super::TUI_PROGRAM, super::super::WEB_PROGRAM])
        .collect();
    // 带上执行位：Unix 上没有执行位的先报「不能跑」，轮不到读加载命令。
    let put = |name: &str, bytes: &[u8]| {
        let file = dir.join(name);
        fs::write(&file, bytes).unwrap();
        crate::dist::layout::executable(&file).unwrap();
    };
    for program in &programs {
        put(program, &legacy_11());
    }
    let named = |found: &[String]| {
        found
            .iter()
            .filter(|problem| problem.contains("LC_DYLD_INFO_ONLY"))
            .count()
    };
    let found = super::super::contents(&repo, &dir, "1.0.0", "macos-aarch64", &[]).unwrap();
    assert_eq!(named(&found), programs.len(), "{found:#?}");
    assert_eq!(super::super::chained(&dir).len(), programs.len());
    let found = super::super::contents(&repo, &dir, "1.0.0", "linux-x86_64", &[]).unwrap();
    assert_eq!(named(&found), 0, "{found:#?}");
    // 只有一个漏了的，只报它一个、报它的名字。
    for program in &programs {
        put(program, &chained_12());
    }
    assert!(super::super::chained(&dir).is_empty());
    put(super::super::WEB_PROGRAM, &legacy_11());
    let left = super::super::chained(&dir);
    assert_eq!(left.len(), 1, "{left:#?}");
    assert!(left[0].starts_with("miyu-web "), "{left:#?}");
}

/// 这个测试程序是苹果的链接器照 cargo 默认的最低系统链的：读得懂它的加载命令，说明读法对得上真的程序（不要求它是 12.0：
/// 平时的编译不设最低系统）。
#[cfg(target_os = "macos")]
#[test]
fn this_program_has_readable_load_commands() {
    let exe = std::env::current_exe().unwrap();
    let loaded = commands(&fs::read(&exe).unwrap()).unwrap();
    println!("{} 的加载命令：{loaded:?}", exe.display());
    assert!(loaded.chained || loaded.legacy, "两种修正信息都没读到");
    let minimum = loaded.minimum.expect("没读到最低系统");
    assert!(minimum >= packed("11.0").unwrap(), "{}", version(minimum));
}
