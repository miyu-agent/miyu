//! 排函数的链接参数（施工 LW-1，`internal/blueprint/perf.md`「排函数」）：环境变量 `MIYU_LINK_ORDER` 指着一份清单（一行
//! 一个函数名，照第一次执行的先后）时，给 `miyu` 这一个程序的链接加上 lld 的 `--symbol-ordering-file`，执行过的函数排在
//! 代码段最前面；没有设置时什么参数都不加，日常的编译、测试、门禁和原来一样。
//!
//! 为什么用构建脚本而不用 `RUSTFLAGS`：`RUSTFLAGS` 一变所有依赖都要重编，还会带到构建脚本和过程宏上；构建脚本的
//! `rustc-link-arg-bin` 只加在 `miyu` 这一个程序的链接上，符号名不变、依赖不重编。清单由 `cargo xtask perf`、
//! `cargo xtask dist` 录好再设这个变量（`xtask/src/order.rs`）。
//!
//! 只做了 Linux（工具链在 x86_64 Linux 上默认用自带的 rust-lld）。别的系统上设了这个变量的让构建失败：只给一句警告的话，
//! 编出来的程序没排，人却以为排了。

use std::ffi::OsStr;
use std::path::Path;

/// 清单的路径从这个环境变量读。
const VARIABLE: &str = "MIYU_LINK_ORDER";

fn main() {
    println!("cargo:rerun-if-env-changed={VARIABLE}");
    let order = std::env::var_os(VARIABLE);
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match lines(&os, order.as_deref()) {
        Ok(lines) => lines.iter().for_each(|line| println!("{line}")),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

/// 给 cargo 的几行指令：没设变量的一行都没有。
///
/// 清单的内容变了也要重链（`rerun-if-changed`），不然 cargo 认为什么都没变，链接的还是上一份。
fn lines(os: &str, order: Option<&OsStr>) -> Result<Vec<String>, String> {
    let Some(order) = order else {
        return Ok(Vec::new());
    };
    if os != "linux" {
        return Err(format!(
            "{VARIABLE} 设了，但排函数这一步只做了 Linux，这次编的目标系统是 {os}：不排的话不要设这个变量"
        ));
    }
    let path = Path::new(order);
    if !path.is_absolute() {
        return Err(format!(
            "{VARIABLE} 要写清单的绝对路径，写的是 {}",
            path.display()
        ));
    }
    let Some(path) = path.to_str() else {
        return Err(format!(
            "{VARIABLE} 的路径 {} 不是 UTF-8，链接参数写不进去",
            path.display()
        ));
    };
    // `-Wl,` 照逗号把后面拆成几个参数，路径里有逗号的会被拆坏。
    if path.contains(',') {
        return Err(format!(
            "{VARIABLE} 的路径 {path} 里有逗号，交给链接器时会被拆开：换一个目录"
        ));
    }
    Ok(vec![
        format!("cargo:rustc-link-arg-bin=miyu=-Wl,--symbol-ordering-file={path}"),
        // 清单里有链接器挪不动的名字（例如 `_start`），不要每个都警告一遍。
        "cargo:rustc-link-arg-bin=miyu=-Wl,--no-warn-symbol-ordering".to_string(),
        format!("cargo:rerun-if-changed={path}"),
    ])
}
