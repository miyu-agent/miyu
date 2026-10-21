//! 测试用的（施工 5-4 上）：cargo 编出来的助手在哪。会话、命令行的测试要真的经助手起命令，在各自的 dev-dependencies
//! 里打开 `testkit`。

use std::path::{Path, PathBuf};

use crate::HELPER;

/// 测试版的助手在 Windows 上收不住时直接起命令（施工 5-9 改上）：设了这个环境变量、助手又是带 `testkit` 编的才这样，正式的助手
/// 里没有这一段。会话、命令行的测试经 [`built_helper`] 拿助手时设上：它们测的是工具、命令行，CI 的 Windows 机器起不了基础
/// 容器，不该因此红。名字不带 `MIYU_`：执行器起命令时把 `MIYU_` 开头的都去掉了。助手自己的测试不设，照旧严格断言拒绝。
pub const PASSTHROUGH: &str = "SANDBOX_TESTKIT_PASSTHROUGH";

/// cargo 编出来的助手：`target/<profile>/miyu-sandbox`，和测试程序所在的 `deps/` 同一层。`cargo test --workspace`
/// 编了它；只测一个包时，先 `cargo build -p miyu-sandbox`。
///
/// # Panics
///
/// 不知道测试程序在哪，或者它不在 `target/<profile>/deps/` 里；那里没有助手。
pub fn built_helper() -> PathBuf {
    #[cfg(windows)]
    // SAFETY: Windows 上读写环境变量由系统串行（Rust 标记 unsafe 是为了 Unix 的 getenv）；只在测试里、值是常量。
    #[allow(unsafe_code, reason = "测试里设一个环境变量（施工 5-9 改上）")]
    unsafe {
        std::env::set_var(PASSTHROUGH, "1");
    }
    let exe = std::env::current_exe().expect("知道测试程序在哪");
    let dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("测试程序在 target/<profile>/deps/ 里");
    let helper = dir.join(HELPER);
    assert!(
        helper.is_file(),
        "{} 不在：先 cargo build -p miyu-sandbox（cargo test --workspace 会编它）",
        helper.display()
    );
    helper
}
