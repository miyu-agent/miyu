//! 脚本的状态目录（施工 X-44、X-44 补，`docs/blueprint/skills.md`「脚本」第 3 条）：照来处、工具名分，场所会话另照场所分；写进去的下一次还在；会话外面的调用不给；建不成的照常跑。Python 的，三个平台都跑。从 `script.rs` 分出来（行数）。

use serde_json::json;

use miyu_tool::Done;

use crate::script::text;
use crate::support::Site;

/// 印出状态目录、在里面数一次跑了几回的脚本（施工 X-44）：没给的印 `unset`。
const COUNT: &[u8] = b"#!/usr/bin/env python3\n# Description: Count.\nimport os, pathlib\nfolder = os.environ.get('MIYU_SCRIPT_STATE_DIR')\nif folder is None:\n    print('unset')\nelse:\n    mark = pathlib.Path(folder) / 'n'\n    n = int(mark.read_text()) + 1 if mark.exists() else 1\n    mark.write_text(str(n))\n    print(n, folder)\n";

/// 状态目录（施工 X-44）：照来处、工具名在数据根的 `state/scripts/` 下面建好交给 `MIYU_SCRIPT_STATE_DIR`，写进去的下一次还在；
/// 换一个来处是另一格；会话外面的调用不给。Python 的，三个平台都跑。
#[tokio::test]
async fn a_script_keeps_its_state_per_source() {
    let site = Site::new();
    site.file("scripts/count.py", COUNT);
    let state = |source: &str| {
        let folder = site.0.join("data/state/scripts").join(source).join("count");
        std::fs::canonicalize(folder).expect("建好了")
    };
    let printed = |done: &Done| text(done).trim_end().to_string();
    let first = site.script_by("scripts/count.py", "~home").await;
    assert!(!first.error, "{}", text(&first));
    assert_eq!(printed(&first), format!("1 {}", state("~home").display()));
    let again = site.script_by("scripts/count.py", "~home").await;
    assert_eq!(
        printed(&again),
        format!("2 {}", state("~home").display()),
        "下一次还在"
    );
    let kit = site.script_by("scripts/count.py", "kit").await;
    assert_eq!(
        printed(&kit),
        format!("1 {}", state("kit").display()),
        "另一个来处另一格"
    );
    let outside = site.script("scripts/count.py", json!({})).await;
    assert_eq!(printed(&outside), "unset", "会话外面的不给");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(state("~home"))
            .expect("在")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700, "只有本人进得去");
    }
}

/// 状态目录建不成的（施工 X-44）：这一次不给，照常跑。
#[tokio::test]
async fn a_script_runs_without_state_when_the_folder_cannot_be_made() {
    let site = Site::new();
    site.file("scripts/count.py", COUNT);
    site.file("data/state/scripts", b"not a folder");
    let done = site.script_by("scripts/count.py", "~home").await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(text(&done), "unset\n");
}

/// 场所会话的状态目录照场所分（施工 X-44 补）：两个群各一格；同一个群再调（新会话）还是那一格、写的还在；本机的会话不分场所。
#[tokio::test]
async fn a_venue_keeps_its_own_state_across_sessions() {
    let site = Site::new();
    site.file("scripts/count.py", COUNT);
    let state = |relative: &str| {
        let folder = site.0.join("data/state/scripts").join(relative);
        std::fs::canonicalize(folder).expect("建好了")
    };
    let printed = |done: &Done| text(done).trim_end().to_string();
    let one = Some("qq:group:1");
    let first = site.script_at("scripts/count.py", "kit", one).await;
    assert!(!first.error, "{}", text(&first));
    let group1 = state("@qq_3agroup_3a1/kit/count");
    assert_eq!(printed(&first), format!("1 {}", group1.display()));
    let again = site.script_at("scripts/count.py", "kit", one).await;
    assert_eq!(
        printed(&again),
        format!("2 {}", group1.display()),
        "同一个群接得上"
    );
    let other = site
        .script_at("scripts/count.py", "kit", Some("qq:group:2"))
        .await;
    let group2 = state("@qq_3agroup_3a2/kit/count");
    assert_eq!(
        printed(&other),
        format!("1 {}", group2.display()),
        "另一个群另一格"
    );
    let local = site
        .script_at("scripts/count.py", "kit", Some("local"))
        .await;
    assert_eq!(
        printed(&local),
        format!("1 {}", state("kit/count").display()),
        "本机的照旧"
    );
}

/// 印出缓存目录的脚本（施工 Y-11 上）。
const CACHED: &[u8] = b"#!/usr/bin/env python3\n# Description: Cached.\nimport os\nprint(os.environ.get('MIYU_SCRIPT_CACHE_DIR', 'unset'))\n";

/// 缓存目录（施工 Y-11 上）：在数据根的 `cache/<来处>/<工具名>/`，建好交真实位置；场所会话多一层场所；会话外面的调用照旧在系统
/// 临时目录下。
#[tokio::test]
async fn a_script_s_cache_is_in_the_data_root() {
    let site = Site::new();
    site.file("scripts/cached.py", CACHED);
    let home = |relative: &str| {
        let folder = site.0.join("data/cache").join(relative);
        std::fs::canonicalize(folder).expect("建好了")
    };
    let printed = |done: &Done| text(done).trim_end().to_string();
    let local = site.script_by("scripts/cached.py", "kit").await;
    assert!(!local.error, "{}", text(&local));
    assert_eq!(printed(&local), home("kit/cached").display().to_string());
    let group = site
        .script_at("scripts/cached.py", "kit", Some("qq:group:1"))
        .await;
    assert_eq!(
        printed(&group),
        home("@qq_3agroup_3a1/kit/cached").display().to_string()
    );
    let outside = site.script("scripts/cached.py", json!({})).await;
    let data = std::fs::canonicalize(site.0.join("data")).expect("在");
    assert!(
        !std::path::Path::new(&printed(&outside)).starts_with(&data),
        "会话外面的照旧在临时目录：{}",
        printed(&outside)
    );
}

/// 印出家目录那四处的脚本（施工 Y-11 补）：没给的印 `unset`。
const HOMED: &[u8] = b"#!/usr/bin/env python3\n# Description: Homed.\nimport os\nfor name in ('PICTURES', 'VIDEOS', 'DOCUMENTS', 'DOWNLOADS'):\n    print(os.environ.get('MIYU_' + name + '_DIR', 'unset'))\n";

/// 家目录那四处（施工 Y-11 补）：本机的会话交属主家目录的 `pictures/`、`videos/`、`documents/`、`downloads/`，建好交真实位置；
/// 场所会话（没有一个人的家，给了就是几个群共用一处）、不知道在哪个场所的、会话外面的调用都不给。
#[tokio::test]
async fn a_local_script_gets_the_owner_s_home_folders() {
    let site = Site::new();
    site.file("scripts/homed.py", HOMED);
    let lines = |done: &Done| -> Vec<String> { text(done).lines().map(str::to_string).collect() };
    let local = site
        .script_at("scripts/homed.py", "kit", Some("local"))
        .await;
    assert!(!local.error, "{}", text(&local));
    let wanted: Vec<String> = ["pictures", "videos", "documents", "downloads"]
        .iter()
        .map(|name| {
            let folder = site.0.join("data").join(name);
            std::fs::canonicalize(folder)
                .expect("建好了")
                .display()
                .to_string()
        })
        .collect();
    assert_eq!(lines(&local), wanted);
    let unset = vec!["unset".to_string(); 4];
    let group = site
        .script_at("scripts/homed.py", "kit", Some("qq:group:1"))
        .await;
    assert_eq!(lines(&group), unset, "场所会话不给");
    let unknown = site.script_at("scripts/homed.py", "kit", None).await;
    assert_eq!(lines(&unknown), unset, "不知道在哪个场所的不给");
    let outside = site.script("scripts/homed.py", json!({})).await;
    assert_eq!(lines(&outside), unset, "会话外面的不给");
}
