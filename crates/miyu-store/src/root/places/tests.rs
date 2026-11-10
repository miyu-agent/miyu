//! 保留的编号（设计 55 第三节第 3 条）：核心自己在包也各占一个 `<编号>/` 的那几层（`etc/`、`var/lib/`、`var/cache/`、
//! `var/log/`、`tmp/`、`run/`）用到的、合编号写法的名字，照各处的方法推出来，和 [`RESERVED`] 一个不多、一个不少。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::super::RESERVED;
use super::*;

/// `path` 在 `layer` 下面的第一层名字。
fn first_below(layer: &Path, path: &Path) -> String {
    let below = path
        .strip_prefix(layer)
        .unwrap_or_else(|_| panic!("{} 不在 {} 下面", path.display(), layer.display()));
    below
        .components()
        .next()
        .expect("在下面")
        .as_os_str()
        .to_string_lossy()
        .into_owned()
}

#[test]
fn reserved_ids_are_exactly_the_names_the_core_uses_beside_packages() {
    let root = DataRoot::at(std::env::temp_dir().join("miyu-reserved"));
    let top = root.path().to_path_buf();
    let x = "x";
    let layers: [(PathBuf, Vec<PathBuf>); 6] = [
        (
            top.join("etc"),
            vec![
                root.config_file(),
                root.trust_file(),
                root.extensions_file(),
                root.knowledge_settings_file(),
                root.secrets_file(),
                root.access_file(),
                root.logins_file(),
                root.sandbox_record_file(),
                root.removed_markers().join(x),
                root.leftover_settings(x),
            ],
        ),
        (
            top.join("var").join("lib"),
            vec![
                root.journal_file(),
                root.salt_dir().join(crate::recall::salt::FILE),
                root.model_state().join(x),
                root.local_packages().join(x),
                root.leftovers_dir().join(crate::packages::leftovers::FILE),
                root.mcp_workdir(x),
                root.blob_grants(x),
            ],
        ),
        (
            top.join("var").join("cache"),
            vec![
                root.index().join(x),
                root.usage_file(),
                root.rates_file(),
                root.config_reference().join(x),
                root.model_cache().join(x),
                root.official_feed(),
                root.other_feeds(),
                root.package_downloads(),
                root.mcp_catalog(x),
                root.provide_cache(x),
                root.script_caches().join(x),
                root.persona_picture_cache(),
                root.mermaid_cache().join(x),
                root.sandbox_probe_dir().join("sandbox-probe.json"),
                root.perf_cache().join(x),
            ],
        ),
        (
            top.join("var").join("log"),
            vec![
                root.logs().join("core.log"),
                root.logs().join("memory.jsonl"),
                root.mcp_stderr(x),
                root.sandbox_report_file(),
            ],
        ),
        (top.join("tmp"), vec![root.unpack_staging().join(x)]),
        // `run/` 里的令牌、套接字的名字在 `miyu-ipc`（它照 [`RESERVED`] 测自己的名字）；锁文件 `core.lock` 带点。
        (
            root.run(),
            vec![root.run().join("token"), root.run().join("socket")],
        ),
    ];
    let mut used = BTreeSet::new();
    for (layer, paths) in &layers {
        for path in paths {
            let name = first_below(layer, path);
            if crate::layers::valid(&name) {
                used.insert(name);
            }
        }
    }
    let reserved: BTreeSet<String> = RESERVED.iter().map(|name| (*name).to_string()).collect();
    assert_eq!(used, reserved);
}

#[test]
fn a_package_takes_one_folder_in_each_layer() {
    let root = DataRoot::at(std::env::temp_dir().join("miyu-reserved"));
    let top = root.path();
    assert_eq!(root.package_state("web"), top.join("var/lib/web"));
    assert_eq!(root.package_logs("web"), top.join("var/log/web"));
    assert_eq!(root.extension_stderr("web"), top.join("var/log/web/stderr"));
    assert_eq!(root.packages().join("web"), top.join("opt/web"));
    assert_eq!(
        root.removed_markers().join("web"),
        top.join("etc/removed/web")
    );
    assert_eq!(
        root.package_places("web"),
        [
            top.join("etc/web"),
            top.join("var/lib/web"),
            top.join("var/cache/web"),
            top.join("var/log/web"),
            top.join("tmp/web"),
            top.join("run/web"),
        ]
    );
}
