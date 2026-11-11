//! 一次调用带的沙盒（`docs/blueprint/session/tools.md` 第 1a 条，施工 5-4 上）：执行器照派出去那一刻实际生效的那一级
//! 写规格。A16 起本人的会话只剩两级（施工 LV-1）：完全放开不进沙盒，只读哪儿都不能写、藏数据根。Unix 上有收紧手段的，
//! 真的经助手跑 `shell`：只读时哪儿都写不进，外面读得到，数据根读不到。

use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(unix)]
use miyu_kernel::event::{Body, ToolResult, ToolStatus};
use miyu_kernel::event::{Level, Permission};
use miyu_kernel::tool::Access;
use miyu_sandbox::{Sandboxed, Spec};
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 假的助手：假工具不起它。
const HELPER: &str = "miyu-sandbox";

/// 真实的位置。
fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).expect("在")
}

/// 在 `cwd` 里造一个会话，沙盒能用，没人能确认：执行命令的假工具 `run` 跑一次，交回它拿到的沙盒。
async fn attached(home: &Home, cwd: &str, permission: Permission) -> Option<Arc<Sandboxed>> {
    let run = Fake::new("run", Access::Execute, Act::Echo);
    let tools = Catalog::new([Arc::clone(&run) as Arc<dyn Tool>]).expect("合写法");
    let script = Script::new([Play::calls(&[("run", "{}")]), Play::Says("好。")]);
    let opening = Opening {
        permission,
        attended: false,
        cwd: cwd.to_string(),
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from(HELPER)),
    };
    let handle = home.create_as(&script, &tools, opening).await;
    turn(&handle).await;
    let calls = run.calls();
    assert_eq!(calls.len(), 1, "跑了一次");
    calls[0]
        .sandbox
        .clone()
        .map(|sandboxed| on_site(home, sandboxed))
}

/// 说一句，等这一轮说完。
async fn turn(handle: &Handle) {
    let mut pushes = watch(handle).await;
    ask(handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

fn permission(level: Level, read_only: bool) -> Permission {
    Permission { level, read_only }
}

/// 日志里的工具结果，照先后。
#[cfg(unix)]
fn results(home: &Home, handle: &Handle) -> Vec<ToolResult> {
    home.log(handle.id())
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn each_level_gets_its_own_spec() {
    let home = Home::outside_temp();
    let work = home.scratch.0.join("work");
    let data = real(home.root.path());
    let cwd = work.to_string_lossy().into_owned();
    let read_only = Sandboxed {
        helper: PathBuf::from(HELPER),
        spec: Spec {
            write: Vec::new(),
            hidden: vec![data],
            read: None,
            reveal: Vec::new(),
            sockets: Vec::new(),
        },
        env: Vec::new(),
        uncached: false,
    };
    for level in [
        permission(Level::Full, true),
        permission(Level::Other("root".to_string()), false),
    ] {
        assert_eq!(
            attached(&home, &cwd, level.clone()).await.as_deref(),
            Some(&read_only),
            "{level:?}：只读，哪儿都不能写；不认识的级别按只读"
        );
    }
    assert_eq!(
        attached(&home, &cwd, permission(Level::Full, false)).await,
        None,
        "完全放开：不进沙盒"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn commands_really_run_in_the_sandbox() {
    let helper = miyu_sandbox::testkit::built_helper();
    if !can_confine(&helper) {
        return;
    }
    let home = Home::outside_temp();
    let work = home.scratch.0.join("work");
    let other = home.scratch.0.join("other");
    std::fs::write(other.join("readable.txt"), "readable\n").expect("写得进");
    std::fs::write(home.root.path().join("marker"), "secret\n").expect("写得进");
    let outside = other.join("outside.txt");
    let marker = home.root.path().join("marker");
    let command = format!(
        "(echo in > inside.txt) 2>/dev/null && echo wrote-inside || echo blocked-inside; \
         (echo out > '{}') 2>/dev/null && echo wrote-outside || echo blocked-outside; \
         cat '{}'; \
         cat '{}' 2>/dev/null || echo blocked-data",
        outside.display(),
        other.join("readable.txt").display(),
        marker.display()
    );
    let output = shell(&home, &helper, &work, &command).await;
    assert_eq!(
        output, "blocked-inside\nblocked-outside\nreadable\nblocked-data\n",
        "只读：哪儿都写不进，外面读得到，数据根读不到"
    );
    assert!(!work.join("inside.txt").exists());
    assert!(!outside.exists());
}

/// 本人只读下私密目录里面的读不到（施工 LV-5：去掉 bubblewrap 一路以前、以后都一样，Linux 上靠 Landlock）；旁边不私密的照常读。
#[cfg(unix)]
#[tokio::test]
async fn a_read_only_command_cannot_read_inside_a_private_dir() {
    let helper = miyu_sandbox::testkit::built_helper();
    if !can_confine(&helper) {
        return;
    }
    let home = Home::outside_temp();
    let work = home.scratch.0.join("work");
    let key = home.scratch.0.join("home/.ssh/id_ed25519");
    let notes = home.scratch.0.join("home/notes.txt");
    std::fs::create_dir_all(key.parent().expect("有上级目录")).expect("建得了目录");
    std::fs::write(&key, "key\n").expect("写得进");
    std::fs::write(&notes, "notes\n").expect("写得进");
    let command = format!(
        "cat '{}' 2>/dev/null || echo blocked-private; \
         (echo x >> '{}') 2>/dev/null && echo wrote-private || echo blocked-write; \
         cat '{}'",
        key.display(),
        key.display(),
        notes.display()
    );
    let output = shell(&home, &helper, &work, &command).await;
    assert_eq!(
        output, "blocked-private\nblocked-write\nnotes\n",
        "只读：私密目录里面读不到、写不进，旁边的照常读"
    );
    assert_eq!(std::fs::read_to_string(&key).expect("还在"), "key\n");
}

/// 这台机器上助手有没有收紧的手段：没有的（例如没有 Landlock 的内核），这个测试到此为止。
#[cfg(unix)]
fn can_confine(helper: &Path) -> bool {
    let probe = miyu_sandbox::probe(helper, std::time::Duration::from_secs(60)).expect("探得了");
    !probe.mechanisms.is_empty()
}

/// 在 `work` 里，只读，经助手 `helper` 跑一条 `shell`，交回给她看的输出。
#[cfg(unix)]
async fn shell(home: &Home, helper: &Path, work: &Path, command: &str) -> String {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来"))
        .expect("合写法");
    let args = serde_json::json!({ "command": command, "description": "Test" }).to_string();
    let script = Script::new([Play::calls(&[("shell", args.as_str())]), Play::Says("好。")]);
    let opening = Opening {
        permission: permission(Level::Full, true),
        attended: false,
        cwd: work.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(helper.to_path_buf()),
    };
    let handle = home.create_as(&script, &tools, opening).await;
    turn(&handle).await;
    let results = results(home, &handle);
    assert_eq!(results.len(), 1, "{results:?}");
    assert_eq!(results[0].status, ToolStatus::Ok, "{results:?}");
    results[0]
        .blocks
        .iter()
        .map(|block| match block {
            miyu_kernel::block::Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}
