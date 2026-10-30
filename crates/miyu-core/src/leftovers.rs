//! 核心起来时收拾上一次崩了留下的（`docs/blueprint/core.md`「起来的先后」）：升级装上的包到一半被杀的放回去（施工 AE-7，
//! [`miyu_store::packages::install::recover`]）。读清单之前做，在这之前没有在装的。

use miyu_store::packages::install;
use miyu_store::root::DataRoot;

use crate::TARGET;

/// 收拾一遍：做了什么、哪里不成的各记一行，不耽误起来。
pub(crate) fn tidy(root: &DataRoot) {
    let recovered = install::recover(&root.packages());
    for id in &recovered.restored {
        tracing::warn!(target: TARGET, package = id.as_str(), "half-done package upgrade put back");
    }
    // 路径不写进运行日志（LG-1）：哪一份照 `packages/` 里点开头的那几个找得到。
    for (_, error) in &recovered.failed {
        tracing::warn!(target: TARGET, error = %error, "package leftover kept");
    }
}
