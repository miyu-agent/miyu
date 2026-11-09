//! 权限策略（施工 4-3 下）：工具报出要碰的路径，会话照边界和级别判放行、问人还是拒绝。施工 5-4 上起读哪儿都放行
//! （数据根除外）。A16 起本人的会话只剩完全放开、只读两级（施工 LV-1）：完全放开读写都不问，只读执行命令照沙盒能不能用。
//!
//! 场地不在系统的临时目录里（[`Home::outside_temp`]）：数据根 `data/`、假的家 `home/`、工作区 `work/`、
//! 边界以外的 `other/`。

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::event::{Body, Event, Level, Permission, Said, ToolResult, ToolStatus};
use miyu_kernel::origin::By;
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 知识库里的文档换一句（施工 K-3 补）。
#[path = "guard/knowledge.rs"]
mod knowledge;

/// 会报路径的三件假工具：读（`read`）、写（`edit`）、执行命令（`run`，不报路径）。
struct Kit {
    read: Arc<Fake>,
    edit: Arc<Fake>,
    run: Arc<Fake>,
}

impl Kit {
    fn new() -> Kit {
        Kit {
            read: Fake::new("read", Access::Read, Act::Echo),
            edit: Fake::new("edit", Access::Write, Act::Echo),
            run: Fake::new("run", Access::Execute, Act::Echo),
        }
    }

    fn catalog(&self) -> Catalog {
        Catalog::new(
            [&self.read, &self.edit, &self.run].map(|tool| Arc::clone(tool) as Arc<dyn Tool>),
        )
        .expect("合写法")
    }
}

/// 场地里的一处，写成她会给的样子（绝对路径）。
fn at(home: &Home, path: &str) -> String {
    home.scratch.0.join(path).to_string_lossy().into_owned()
}

/// 调一件工具，参数里是一条路径。
fn one(tool: &str, path: &str) -> Play {
    Play::calls(&[(tool, &serde_json::json!({ "path": path }).to_string())])
}

/// 在工作区 `work/` 里造一个会话：级别 `level`、只读 `read_only`、有没有人能确认 `attended`；沙盒用不了。
async fn session(
    home: &Home,
    script: &Script,
    kit: &Kit,
    level: Level,
    read_only: bool,
    attended: bool,
) -> Handle {
    let opening = Opening {
        permission: Permission { level, read_only },
        attended,
        cwd: at(home, "work"),
        dirs: Vec::new(),
        sandbox: None,
    };
    home.create_as(script, &kit.catalog(), opening).await
}

/// 同 [`session`]，这台机器上的沙盒能用：假工具不起助手，随便一条路径。
async fn sandboxed(home: &Home, script: &Script, kit: &Kit, read_only: bool) -> Handle {
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only,
        },
        attended: false,
        cwd: at(home, "work"),
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from("miyu-sandbox")),
    };
    home.create_as(script, &kit.catalog(), opening).await
}

/// 说一句，等这一轮说完。
async fn turn(handle: &Handle) {
    let mut pushes = watch(handle).await;
    ask(handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 日志里的第一条工具结果，和记它的是谁。
fn first_result(log: &[Event]) -> (ToolResult, By) {
    log.iter()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some((result.clone(), event.by.clone())),
            _ => None,
        })
        .expect("有一条工具结果")
}

/// 结果里的字。
fn text(result: &ToolResult) -> String {
    result
        .blocks
        .iter()
        .map(|block| match block {
            miyu_kernel::block::Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

/// 是权限策略拒绝的。
fn by_permissions(by: &By) -> bool {
    matches!(by, By::Module(module) if module.id.as_str() == "permissions")
}

fn file(home: &Home, path: &str, text: &str) {
    let path = home.scratch.0.join(path);
    std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
    std::fs::write(path, text).expect("写得进");
}

#[tokio::test]
async fn reads_and_writes_inside_the_workspace_run() {
    let home = Home::outside_temp();
    file(&home, "work/a.txt", "a");
    let kit = Kit::new();
    let script = Script::new([
        one("read", "a.txt"),
        Play::Says("好。"),
        one("edit", "new.txt"),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script, &kit, Level::Full, false, false).await;
    turn(&handle).await;
    assert_eq!(
        kit.read.calls().len(),
        1,
        "相对路径照这一轮的工作目录接，在工作区里"
    );
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-2", say("再来")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert_eq!(kit.edit.calls().len(), 1, "工作区里还不存在的文件也能写");
}

#[tokio::test]
async fn a_read_outside_runs_at_every_level() {
    let home = Home::outside_temp();
    file(&home, "other/b.txt", "b");
    for read_only in [false, true] {
        let kit = Kit::new();
        let script = Script::new([one("read", &at(&home, "other/b.txt")), Play::Says("好。")]);
        let handle = session(&home, &script, &kit, Level::Full, read_only, false).await;
        turn(&handle).await;
        assert_eq!(
            kit.read.calls().len(),
            1,
            "只读 {read_only}：读哪儿都放行，没人能确认也不用问"
        );
    }
}

#[tokio::test]
async fn the_data_root_is_refused_at_every_level() {
    let home = Home::outside_temp();
    let token = home.root.run().join("token-for-test");
    std::fs::write(&token, "secret").expect("写得进");
    let token = token.to_string_lossy().into_owned();
    for read_only in [false, true] {
        let kit = Kit::new();
        let script = Script::new([one("read", &token), Play::Says("好。")]);
        let handle = session(&home, &script, &kit, Level::Full, read_only, true).await;
        turn(&handle).await;
        let (result, by) = first_result(&home.log(handle.id()));
        assert_eq!(result.status, ToolStatus::Denied, "只读 {read_only}");
        assert!(by_permissions(&by), "{by:?}");
        assert!(
            text(&result).contains("inside Miyu's own data"),
            "{}",
            text(&result)
        );
        // 给人看的说法也记下了（施工 4-5 上）。
        assert_eq!(
            result.human,
            Some(Said::new("core/permissions/forbidden").with("path", token.as_str()))
        );
        assert!(kit.read.calls().is_empty(), "只读 {read_only}");
    }
}

#[tokio::test]
async fn full_access_writes_outside_without_asking() {
    let home = Home::outside_temp();
    file(&home, "other/b.txt", "b");
    let kit = Kit::new();
    let script = Script::new([one("edit", &at(&home, "other/b.txt")), Play::Says("好。")]);
    let handle = session(&home, &script, &kit, Level::Full, false, false).await;
    turn(&handle).await;
    assert_eq!(kit.edit.calls().len(), 1);
}

/// 执行命令（A16，施工 LV-1）：完全放开不进沙盒，沙盒能不能用都照跑；只读沙盒能用就在沙盒里跑、不问，用不了的每条都问人，
/// 这个会话没人能确认，就拒绝。
#[tokio::test]
async fn commands_go_by_whether_the_sandbox_can_be_used() {
    let home = Home::outside_temp();
    for read_only in [false, true] {
        // 沙盒能用：两级都不问，没人能确认也跑（施工 5-4 上）。
        let kit = Kit::new();
        let script = Script::new([Play::calls(&[("run", "{}")]), Play::Says("好。")]);
        let handle = sandboxed(&home, &script, &kit, read_only).await;
        turn(&handle).await;
        assert_eq!(kit.run.calls().len(), 1, "只读 {read_only}：沙盒能用");
        // 用不了：完全放开照跑，只读问人、没人能确认就拒绝。
        let kit = Kit::new();
        let script = Script::new([Play::calls(&[("run", "{}")]), Play::Says("好。")]);
        let handle = session(&home, &script, &kit, Level::Full, read_only, false).await;
        turn(&handle).await;
        let (result, _) = first_result(&home.log(handle.id()));
        let expected = if read_only {
            ToolStatus::Denied
        } else {
            ToolStatus::Ok
        };
        assert_eq!(result.status, expected, "只读 {read_only}：沙盒用不了");
        assert_eq!(
            kit.run.calls().len(),
            usize::from(!read_only),
            "只读 {read_only}：沙盒用不了"
        );
    }
}

#[tokio::test]
async fn several_paths_go_by_the_strictest() {
    let home = Home::outside_temp();
    file(&home, "work/a.txt", "a");
    file(&home, "other/b.txt", "b");
    let token = home.root.run().join("token-for-test");
    std::fs::write(&token, "secret").expect("写得进");
    let kit = Kit::new();
    let args = serde_json::json!({ "paths": [at(&home, "work/a.txt"), token.to_string_lossy(), at(&home, "other/b.txt")] }).to_string();
    let script = Script::new([Play::calls(&[("read", &args)]), Play::Says("好。")]);
    let handle = session(&home, &script, &kit, Level::Full, false, true).await;
    turn(&handle).await;
    let (result, by) = first_result(&home.log(handle.id()));
    assert_eq!(
        result.status,
        ToolStatus::Denied,
        "有一条在数据根里：拒绝，不问人"
    );
    assert!(by_permissions(&by));
}

#[cfg(unix)]
#[tokio::test]
async fn a_link_to_nowhere_is_refused_and_says_why() {
    let home = Home::outside_temp();
    std::os::unix::fs::symlink(
        home.scratch.0.join("nowhere"),
        home.scratch.0.join("work/dead"),
    )
    .expect("造得了链接");
    let kit = Kit::new();
    let script = Script::new([one("read", "dead/x.txt"), Play::Says("好。")]);
    let handle = session(&home, &script, &kit, Level::Full, false, true).await;
    turn(&handle).await;
    let (result, by) = first_result(&home.log(handle.id()));
    assert_eq!(result.status, ToolStatus::Denied);
    assert!(by_permissions(&by));
    assert_eq!(
        text(&result),
        "Can't tell where \"dead/x.txt\" points: a link on the path points nowhere.\n"
    );
    assert_eq!(
        result.human,
        Some(
            Said::new("core/permissions/unresolvable")
                .with("path", "dead/x.txt")
                .with("reason", "a link on the path points nowhere")
        )
    );
}

/// `~` 开头的照会话的家目录换（场地的假家目录 `home/`）：换得成真实的位置，完全放开照写，工具拿到的判过的位置就是家目录下
/// 那一个。说不清在哪的才是权限策略拒的。
#[tokio::test]
async fn a_tilde_path_follows_the_home() {
    let home = Home::outside_temp();
    file(&home, "home/notes.txt", "notes");
    let kit = Kit::new();
    let script = Script::new([one("edit", "~/notes.txt"), Play::Says("好。")]);
    let handle = session(&home, &script, &kit, Level::Full, false, false).await;
    turn(&handle).await;
    let (result, _) = first_result(&home.log(handle.id()));
    assert_eq!(result.status, ToolStatus::Ok, "{}", text(&result));
    let [call] = kit.edit.calls().try_into().expect("跑了一次");
    let notes = std::fs::canonicalize(home.home.join("notes.txt")).expect("在");
    assert_eq!(call.checked.as_deref(), Some(&[notes][..]));
}

/// `trash` 删的是链接本身，权限策略判的也是链接本身（施工 4-9 再补二）：工作区里的链接，指向不存在处的、指进数据根的、
/// 指到外面的，都照工作区里的写放行，没人能确认也不用问；删掉的是链接，它指的东西不动。回收站在场地的假家目录里，
/// 只在 Linux 上跑：别的平台进的是系统真的回收站。
#[cfg(target_os = "linux")]
#[tokio::test]
async fn trash_is_judged_on_the_link_itself() {
    use std::os::unix::fs::symlink;

    let home = Home::outside_temp();
    let work = home.scratch.0.join("work");
    file(&home, "outside.txt", "out");
    let secret = home.root.path().join("marker");
    std::fs::write(&secret, "secret").expect("写得进");
    symlink(home.scratch.0.join("nowhere"), work.join("dead")).expect("造得了链接");
    symlink(home.scratch.0.join("outside.txt"), work.join("out")).expect("造得了链接");
    symlink(&secret, work.join("data")).expect("造得了链接");
    let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来"))
        .expect("合写法");
    let script = Script::new([
        Play::calls(&[
            ("trash", r#"{"file_path":"dead"}"#),
            ("trash", r#"{"file_path":"out"}"#),
            ("trash", r#"{"file_path":"data"}"#),
        ]),
        Play::Says("删了。"),
    ]);
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only: false,
        },
        attended: false,
        cwd: work.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
    };
    let handle = home.create_as(&script, &tools, opening).await;
    turn(&handle).await;
    let results: Vec<ToolResult> = home
        .log(handle.id())
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect();
    assert_eq!(results.len(), 3);
    for result in &results {
        assert_eq!(result.status, ToolStatus::Ok, "{}", text(result));
    }
    for name in ["dead", "out", "data"] {
        assert!(
            std::fs::symlink_metadata(work.join(name)).is_err(),
            "{name} 删了"
        );
    }
    assert!(home.scratch.0.join("outside.txt").exists(), "指的东西不动");
    assert!(secret.exists(), "数据根里的不动");
}
