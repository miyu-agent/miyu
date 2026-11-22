//! 只在 macOS 上跑的真探针（施工 MO-2）：现编 `crates/miyu-perf/probe/pagetrace.c` 和一个小的 C 程序，照 [`Probed`] 的
//! 环境变量起它，再照 [`read`] 读记录、换成函数名。它要 `cc`（Xcode 的命令行工具）和苹果的 `nm`，默认不跑；在 Mac 上这样跑：
//!
//! ```text
//! cargo test -p miyu-perf --release -- --ignored real_probe
//! ```
//!
//! 产物放在 `CARGO_TARGET_DIR`（没设的是仓库的 `target/`）下的 `miyu-perf-probe-mac/`，不放系统的临时目录。

use std::path::{Path, PathBuf};
use std::process::Command;

use super::read;
use crate::order::listing::{Listing, Symbols};
use crate::order::nm;
use crate::sandbox::Probed;

/// 被测的小程序：`main` 先后调三个不内联的函数，四个都对齐到一页（Apple 芯片上一页 16 kB），各自占一页。探针只关
/// `__text` 整页的部分，排在最后的 `main` 落在末尾那不满一页的部分上，记不到（2026-11-23 在一台 M1 MacBook 上跑过）；
/// 前三个各自的页照调用的先后记到。
const PROGRAM: &str = r#"
#include <stdio.h>
#define PAGE __attribute__((noinline, aligned(16384)))
PAGE int first(int x) { return x + 1; }
PAGE int second(int x) { return x * 2; }
PAGE int third(int x) { return x - 3; }
PAGE int main(void) { printf("%d\n", third(second(first(20)))); return 0; }
"#;

fn run(command: &mut Command, what: &str) {
    let output = command
        .output()
        .unwrap_or_else(|e| panic!("{what}拉不起：{e}"));
    assert!(
        output.status.success(),
        "{what}：{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "要 macOS 的 cc 和 nm，在 Mac 上手工跑"]
fn real_probe_records_the_functions_of_a_small_program() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map_or_else(|| manifest.join("../../target"), PathBuf::from);
    let dir = target.join("miyu-perf-probe-mac");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("删得掉");
    }
    std::fs::create_dir_all(&dir).expect("建得了");
    let library = dir.join("pagetrace.dylib");
    run(
        Command::new("cc")
            .args(["-O2", "-dynamiclib", "-Wall", "-Wextra", "-Werror", "-o"])
            .arg(&library)
            .arg(manifest.join("probe/pagetrace.c")),
        "编探针",
    );
    let source = dir.join("small.c");
    std::fs::write(&source, PROGRAM).expect("写得进");
    let program = dir.join("small");
    run(
        Command::new("cc")
            .args(["-O1", "-o"])
            .arg(&program)
            .arg(&source),
        "编小程序",
    );
    let prefix = dir.join("small.pages");
    let probed = Probed {
        library: &library,
        out: &prefix,
        program: &program,
    };
    run(
        Command::new(&program).envs(probed.vars().expect("拼得出")),
        "带探针跑小程序",
    );
    let symbols = Symbols::parse(&nm(&program).expect("读得了符号表"));
    let mut listing = Listing::new(&symbols);
    let got = read(&prefix, &program, &mut listing).expect("有记录");
    assert_eq!(got.files, 1, "{got:?}");
    assert_eq!(listing.names(), ["_first", "_second", "_third"]);
    std::fs::remove_dir_all(&dir).expect("删得掉");
}
