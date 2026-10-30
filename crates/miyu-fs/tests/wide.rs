//! 工作目录太宽（施工 4-3 下）：根目录、落在数据根里而不在工作区 `workspace/` 里的太宽；家目录、包含数据根的不算（施工
//! 5-14 改、SW-7），加进来的目录、场所会话照旧照宽的算（`broad`）。太宽的工作目录对应的工作区是这个会话的默认工作区，别的就是它自己
//! （施工 5-14）。

use std::path::{Path, PathBuf};

use miyu_fs::{broad, too_wide, workspace_of};

fn root() -> PathBuf {
    PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" })
}

#[test]
fn what_counts_as_broad() {
    let home = root().join("home").join("me");
    let data = home.join(".miyu");
    let own = data.join("home").join("admin").join("workspace");
    let wide = |dir: &Path| broad(dir, Some(&home), &data, &own);
    assert!(wide(&home), "家目录");
    assert!(wide(&root()), "根目录");
    assert!(wide(&root().join("home")), "包含数据根");
    assert!(wide(&data), "数据根本身");
    assert!(wide(&data.join("state")), "落在数据根里");
    assert!(!wide(&own), "账号自己的工作区");
    assert!(!wide(&own.join("sub")), "账号工作区里面");
    assert!(!wide(&home.join("proj")), "项目目录");
    // 家目录包含数据根：家目录读不出来也认得出宽。
    assert!(broad(&home, None, &data, &own));
    // 数据根在别处（MIYU_HOME）：家目录只有读得出来才认得。
    let data = root().join("srv").join("miyu");
    let own = data.join("home").join("admin").join("workspace");
    assert!(broad(&home, Some(&home), &data, &own));
    assert!(!broad(&home, None, &data, &own));
}

/// 施工 5-14 改（2026-10-22 项目主人定）：家目录、包含数据根的不再太宽（Windows 施工 SW-7 跟上）；根目录、落在数据根里的哪儿都太宽。
#[test]
fn what_counts_as_too_wide() {
    let home = root().join("home").join("me");
    let data = home.join(".miyu");
    let own = data.join("home").join("admin").join("workspace");
    let wide = |dir: &Path| too_wide(dir, Some(&home), &data, &own);
    assert!(wide(&root()), "根目录");
    assert!(wide(&data), "数据根本身");
    assert!(wide(&data.join("state")), "落在数据根里");
    assert!(!wide(&own), "账号自己的工作区");
    assert!(!wide(&home.join("proj")), "项目目录");
    assert!(!wide(&home), "家目录");
    assert!(!wide(&root().join("home")), "包含数据根");
}

#[test]
fn a_too_wide_directory_works_in_the_session_s_own_folder() {
    let home = root().join("home").join("me");
    let data = home.join(".miyu");
    let own = data.join("home").join("admin").join("workspace");
    let fallback = own.join("0199-session");
    let of = |cwd: &Path| workspace_of(cwd, Some(&home), &data, &own, &fallback);
    let home_is = if cfg!(windows) {
        fallback.clone()
    } else {
        home.clone()
    };
    assert_eq!(of(&home), home_is, "家目录：Windows 上退回，别处就是它自己");
    assert_eq!(of(&root()), fallback, "根目录");
    assert_eq!(of(&data.join("state")), fallback, "落在数据根里");
    assert_eq!(
        of(&home.join("proj")),
        home.join("proj"),
        "项目目录就是它自己"
    );
    assert_eq!(of(&own), own, "账号自己的工作区就是它自己");
    assert_eq!(of(&fallback), fallback);
}
