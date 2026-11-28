//! 读导入表、认 VC++ 运行库：照 PE 格式现搭的小文件测；MSVC 的 Windows 上再读这个测试程序自己，证明仓库的 `+crt-static` 生效了。

use super::*;

/// 一个只有一节（`.idata`）的 64 位 PE 文件，导入表照先后列 `dlls`。
fn pe(dlls: &[&str]) -> Vec<u8> {
    const RAW: usize = 0x200;
    const VA: u32 = 0x1000;
    let put16 = |file: &mut [u8], at: usize, value: u16| {
        file[at..at + 2].copy_from_slice(&value.to_le_bytes());
    };
    let put32 = |file: &mut [u8], at: usize, value: u32| {
        file[at..at + 4].copy_from_slice(&value.to_le_bytes());
    };
    // 描述符在前（最后一条全是零），名字跟在后面。
    let mut data = vec![0u8; (dlls.len() + 1) * DESCRIPTOR];
    for (index, dll) in dlls.iter().enumerate() {
        let name = VA + u32::try_from(data.len()).unwrap();
        put32(&mut data, index * DESCRIPTOR + 12, name);
        data.extend_from_slice(dll.as_bytes());
        data.push(0);
    }
    let size = u32::try_from(data.len()).unwrap();
    let mut file = vec![0u8; RAW];
    file[..2].copy_from_slice(b"MZ");
    put32(&mut file, 0x3c, 0x40);
    file[0x40..0x44].copy_from_slice(b"PE\0\0");
    put16(&mut file, 0x44, 0x8664);
    put16(&mut file, 0x46, 1);
    put16(&mut file, 0x54, 240);
    let optional = 0x58;
    put16(&mut file, optional, 0x20b);
    put32(&mut file, optional + 108, 16);
    put32(&mut file, optional + 112 + 8, VA);
    put32(&mut file, optional + 112 + 12, size);
    let section = optional + 240;
    file[section..section + 6].copy_from_slice(b".idata");
    put32(&mut file, section + 8, size);
    put32(&mut file, section + 12, VA);
    put32(&mut file, section + 16, size);
    put32(&mut file, section + 20, u32::try_from(RAW).unwrap());
    file.extend(data);
    file
}

/// 测试的临时目录，用完删掉。
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("xtask-crt-{name}-{}", std::process::id()));
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
fn every_dll_comes_out_in_order() {
    let dlls = [
        "KERNEL32.dll",
        "VCRUNTIME140.dll",
        "api-ms-win-crt-runtime-l1-1-0.dll",
    ];
    assert_eq!(imports(&pe(&dlls)).unwrap(), dlls);
    assert_eq!(imports(&pe(&[])).unwrap(), Vec::<String>::new());
}

#[test]
fn only_the_vc_runtime_counts() {
    for dll in [
        "VCRUNTIME140.dll",
        "vcruntime140_1.dll",
        "MSVCP140.dll",
        "msvcp140_atomic_wait.dll",
    ] {
        assert!(needs_runtime(dll), "{dll}");
    }
    // 系统自带的：通用 C 运行库、系统的 msvcrt、内核。
    for dll in [
        "api-ms-win-crt-heap-l1-1-0.dll",
        "ucrtbase.dll",
        "msvcrt.dll",
        "KERNEL32.dll",
        "bcryptprimitives.dll",
    ] {
        assert!(!needs_runtime(dll), "{dll}");
    }
}

#[test]
fn what_is_not_a_whole_pe_file_is_an_error() {
    assert!(imports(b"").is_err());
    assert!(imports(b"hello, world").is_err());
    let mut no_signature = pe(&["KERNEL32.dll"]);
    no_signature[0x40] = b'X';
    assert!(imports(&no_signature).is_err());
    // 名字写到一半就截断了。
    let whole = pe(&["KERNEL32.dll"]);
    assert!(imports(&whole[..whole.len() - 3]).is_err());
}

#[test]
fn a_program_needing_the_runtime_is_named_with_its_dlls() {
    let scratch = Scratch::new("named");
    let bad = scratch.0.join("bad.exe");
    fs::write(
        &bad,
        pe(&["KERNEL32.dll", "VCRUNTIME140.dll", "MSVCP140.dll"]),
    )
    .unwrap();
    let said = problem(&bad, "miyu.exe").unwrap();
    assert!(
        said.starts_with("miyu.exe 要 VCRUNTIME140.dll、MSVCP140.dll："),
        "{said}"
    );
    let good = scratch.0.join("good.exe");
    fs::write(
        &good,
        pe(&["KERNEL32.dll", "api-ms-win-crt-heap-l1-1-0.dll"]),
    )
    .unwrap();
    assert_eq!(problem(&good, "miyu.exe"), None);
    let text = scratch.0.join("text.exe");
    fs::write(&text, "program").unwrap();
    assert!(problem(&text, "miyu.exe").unwrap().contains("导入表读不了"));
}

/// 查包照它查每个 Windows 程序；别的系统的包不查。
#[test]
fn the_windows_package_checks_every_program() {
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
    // 带上执行位：Unix 上没有执行位的先报「不能跑」，轮不到读导入表。
    let put = |name: String| {
        let file = dir.join(name);
        fs::write(&file, pe(&["VCRUNTIME140.dll"])).unwrap();
        crate::dist::layout::executable(&file).unwrap();
    };
    for program in &programs {
        put(format!("{program}.exe"));
    }
    let found = super::super::contents(&repo, &dir, "1.0.0", "windows-x86_64", &[]).unwrap();
    let named: Vec<&String> = found
        .iter()
        .filter(|problem| problem.contains("VCRUNTIME140.dll"))
        .collect();
    assert_eq!(named.len(), programs.len(), "{found:#?}");
    for program in &programs {
        put((*program).to_string());
    }
    let found = super::super::contents(&repo, &dir, "1.0.0", "linux-x86_64", &[]).unwrap();
    assert!(
        found.iter().all(|problem| !problem.contains("VCRUNTIME")),
        "{found:#?}"
    );
}

/// 这个测试程序和发出去的程序照同一份 `.cargo/config.toml` 编：MSVC 的 Windows 上它也不该要 VC++ 运行库。仓库没给 MSVC 目标
/// 加 `+crt-static` 时这一条红。
#[cfg(all(windows, target_env = "msvc"))]
#[test]
fn this_program_does_not_need_the_vc_runtime() {
    let exe = std::env::current_exe().unwrap();
    let dlls = imports(&fs::read(&exe).unwrap()).unwrap();
    println!("{} 的导入表：{dlls:?}", exe.display());
    assert!(
        dlls.iter()
            .any(|dll| dll.eq_ignore_ascii_case("kernel32.dll")),
        "读到的导入表不像 Windows 程序的：{dlls:?}"
    );
    let runtime: Vec<&String> = dlls.iter().filter(|dll| needs_runtime(dll)).collect();
    assert!(runtime.is_empty(), "要了 VC++ 运行库：{runtime:?}");
}
