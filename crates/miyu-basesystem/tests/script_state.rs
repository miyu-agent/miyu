//! 脚本的状态目录（施工 X-44、X-44 补，`docs/blueprint/skills.md`「脚本」第 3 条）：照会话属主、来处、工具名分，场所会话另照
//! 场所分；写进去的下一次还在；会话外面的调用不给；建不成的照常跑。Python 的，三个平台都跑。从 `script.rs` 分出来（行数）。

use serde_json::json;

use miyu_tool::Done;

use crate::script::text;
use crate::support::Site;

/// 印出状态目录、在里面数一次跑了几回的脚本（施工 X-44）：没给的印 `unset`。
const COUNT: &[u8] = b"#!/usr/bin/env python3\n# Description: Count.\nimport os, pathlib\nfolder = os.environ.get('MIYU_SCRIPT_STATE_DIR')\nif folder is None:\n    print('unset')\nelse:\n    mark = pathlib.Path(folder) / 'n'\n    n = int(mark.read_text()) + 1 if mark.exists() else 1\n    mark.write_text(str(n))\n    print(n, folder)\n";

/// 状态目录（施工 X-44）：照会话属主、来处、工具名在数据根的 `state/scripts/` 下面建好交给 `MIYU_SCRIPT_STATE_DIR`，写进去的
/// 下一次还在；换一个属主、换一个来处都是另一格；会话外面的调用（没有属主）不给。Python 的，三个平台都跑。
#[tokio::test]
async fn a_script_keeps_its_state_per_owner_and_source() {
    let site = Site::new();
    site.file("scripts/count.py", COUNT);
    let state = |owner: &str, source: &str| {
        let folder = site
            .0
            .join("data/state/scripts")
            .join(owner)
            .join(source)
            .join("count");
        std::fs::canonicalize(folder).expect("建好了")
    };
    let printed = |done: &Done| text(done).trim_end().to_string();
    let first = site.script_by("scripts/count.py", "~home", "alice").await;
    assert!(!first.error, "{}", text(&first));
    assert_eq!(
        printed(&first),
        format!("1 {}", state("alice", "~home").display())
    );
    let again = site.script_by("scripts/count.py", "~home", "alice").await;
    assert_eq!(
        printed(&again),
        format!("2 {}", state("alice", "~home").display()),
        "下一次还在"
    );
    let bob = site.script_by("scripts/count.py", "~home", "bob").await;
    assert_eq!(
        printed(&bob),
        format!("1 {}", state("bob", "~home").display()),
        "另一个属主另一格"
    );
    let kit = site.script_by("scripts/count.py", "kit", "alice").await;
    assert_eq!(
        printed(&kit),
        format!("1 {}", state("alice", "kit").display()),
        "另一个来处另一格"
    );
    let outside = site.script("scripts/count.py", json!({})).await;
    assert_eq!(printed(&outside), "unset", "没有属主的不给");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(state("alice", "~home"))
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
    let done = site.script_by("scripts/count.py", "~home", "alice").await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(text(&done), "unset\n");
}

/// 场所会话的状态目录照场所分（施工 X-44 补）：两个群各一格；同一个群再调（新会话）还是那一格、写的还在；本机的会话照旧是账号
/// 那一层。
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
    let first = site
        .script_at("scripts/count.py", "kit", "bridge", one)
        .await;
    assert!(!first.error, "{}", text(&first));
    let group1 = state("bridge/@qq_3agroup_3a1/kit/count");
    assert_eq!(printed(&first), format!("1 {}", group1.display()));
    let again = site
        .script_at("scripts/count.py", "kit", "bridge", one)
        .await;
    assert_eq!(
        printed(&again),
        format!("2 {}", group1.display()),
        "同一个群接得上"
    );
    let other = site
        .script_at("scripts/count.py", "kit", "bridge", Some("qq:group:2"))
        .await;
    let group2 = state("bridge/@qq_3agroup_3a2/kit/count");
    assert_eq!(
        printed(&other),
        format!("1 {}", group2.display()),
        "另一个群另一格"
    );
    let local = site
        .script_at("scripts/count.py", "kit", "alice", Some("local"))
        .await;
    assert_eq!(
        printed(&local),
        format!("1 {}", state("alice/kit/count").display()),
        "本机的照旧"
    );
}
