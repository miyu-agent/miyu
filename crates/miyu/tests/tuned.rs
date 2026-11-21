//! 核心带着「线程缓存少留」的那一项环境变量跑（施工 LW-12，`internal/blueprint/heap.md`「线程缓存」）：直接运行 `miyu core`
//! （没带着的）也补得上，人自己设的别的项留着、原来的那一份记在 `MIYU_GLIBC_TUNABLES` 里；人自己写了留几块的照人写的。只有
//! glibc 上有这回事，别的平台什么都不带。

use std::collections::BTreeMap;
use std::process::Command;

use crate::support::{Home, within};
use miyu_ipc::connect_or_start;

/// 拉起 `core`，等它好了，交回它环境里名字带 `TUNABLES` 的那几项。
async fn tunables(home: &Home, core: impl Fn() -> Command) -> BTreeMap<String, String> {
    let (connection, _) = within("拉起", connect_or_start(&home.root, core))
        .await
        .expect("拉得起");
    let pid = home.core_pid().expect("运行日志里有进程号");
    let found = environment(&pid);
    drop(connection);
    home.until_stopped().await;
    found
}

/// 进程 `pid` 的环境里名字带 `TUNABLES` 的那几项；读不到的平台是空的。
fn environment(pid: &str) -> BTreeMap<String, String> {
    let raw = std::fs::read(format!("/proc/{pid}/environ")).unwrap_or_default();
    raw.split(|byte| *byte == 0)
        .filter_map(|pair| {
            let pair = String::from_utf8_lossy(pair);
            let (name, value) = pair.split_once('=')?;
            name.contains("TUNABLES")
                .then(|| (name.to_string(), value.to_string()))
        })
        .collect()
}

/// 要带上的那几项：glibc 上有，别的平台没有。
fn expected(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    match cfg!(all(target_os = "linux", target_env = "gnu")) {
        true => pairs
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect(),
        false => BTreeMap::new(),
    }
}

#[tokio::test]
async fn a_core_run_directly_picks_up_the_setting() {
    let home = Home::new();
    let found = tunables(&home, || {
        let mut core = home.core();
        core.env_remove("GLIBC_TUNABLES")
            .env_remove("MIYU_GLIBC_TUNABLES");
        core
    })
    .await;
    assert_eq!(
        found,
        expected(&[
            ("GLIBC_TUNABLES", "glibc.malloc.tcache_count=4"),
            ("MIYU_GLIBC_TUNABLES", ""),
        ])
    );
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[tokio::test]
async fn what_the_person_set_is_kept_and_remembered() {
    let home = Home::new();
    let found = tunables(&home, || {
        let mut core = home.core();
        core.env("GLIBC_TUNABLES", "glibc.malloc.arena_max=4")
            .env_remove("MIYU_GLIBC_TUNABLES");
        core
    })
    .await;
    assert_eq!(
        found,
        expected(&[
            (
                "GLIBC_TUNABLES",
                "glibc.malloc.arena_max=4:glibc.malloc.tcache_count=4"
            ),
            ("MIYU_GLIBC_TUNABLES", "glibc.malloc.arena_max=4"),
        ])
    );
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[tokio::test]
async fn a_count_the_person_wrote_is_left_alone() {
    let home = Home::new();
    let found = tunables(&home, || {
        let mut core = home.core();
        core.env("GLIBC_TUNABLES", "glibc.malloc.tcache_count=0")
            .env_remove("MIYU_GLIBC_TUNABLES");
        core
    })
    .await;
    assert_eq!(
        found,
        expected(&[("GLIBC_TUNABLES", "glibc.malloc.tcache_count=0")])
    );
}
