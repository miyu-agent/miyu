//! 照场地比沙盒（施工 5-17 上）。

use std::path::PathBuf;
use std::sync::Arc;

use super::Home;

/// 只留这个场地里藏的（施工 5-17 上）：照这台机器的环境变量另藏的私密目录（CI 的 Windows 设了 `AZURE_CONFIG_DIR`）不在
/// 场地里，测试不看它们。
pub fn on_site(
    home: &Home,
    sandboxed: Arc<miyu_sandbox::Sandboxed>,
) -> Arc<miyu_sandbox::Sandboxed> {
    let roots: Vec<PathBuf> = [
        home.root.path(),
        home.home.as_path(),
        home.scratch.0.as_path(),
    ]
    .into_iter()
    .map(|place| std::fs::canonicalize(place).unwrap_or_else(|_| place.to_path_buf()))
    .collect();
    let mut sandboxed = (*sandboxed).clone();
    sandboxed
        .spec
        .hidden
        .retain(|dir| roots.iter().any(|root| dir.starts_with(root)));
    Arc::new(sandboxed)
}
