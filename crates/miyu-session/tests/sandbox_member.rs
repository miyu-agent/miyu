//! 成员和场所会话的沙盒（施工 MB-1，2026-10-24 项目主人定：成员能跑命令，只在他自己的沙盒里）：真的经助手跑 `shell`。
//! 读得到自己的工作区和程序跑起来要的系统文件（证书、域名解析、`/etc/hosts`、`git`）；读不到别的成员的家目录、数据根、
//! 跑核心的那个用户的家目录、机器上别处的文件、`/etc` 里名单外的、`/opt`；要出沙盒的拒、不问人，拒的话里没有核心那边的绝对
//! 路径。关不住读的平台（Windows）上不跑，回 `no-commands`。没有收紧手段的机器到此为止。

use std::path::{Path, PathBuf};

use miyu_kernel::event::{Body, Level, Permission, ToolResult, ToolStatus};
use miyu_kernel::id::VenueId;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

/// 哪种关起来的会话。
#[derive(Debug, Clone, Copy)]
enum Kind {
    /// 成员的本机会话：工作目录是他自己的工作区。
    Member,
    /// 群会话：工作目录是场所的工作区（场地里的 `work/`）。
    Group,
}

/// 在这种会话里经助手跑一条 `shell`（`args` 是参数），交回结果。
async fn shell(home: &Home, kind: Kind, cwd: &Path, args: serde_json::Value) -> ToolResult {
    shell_as(home, kind, cwd, args, None).await
}

/// 同 [`shell`]，权限级别完全放开、这一轮由 `by` 开口（没有的照本机的人）。
async fn shell_as(
    home: &Home,
    kind: Kind,
    cwd: &Path,
    args: serde_json::Value,
    by: Option<miyu_kernel::origin::By>,
) -> ToolResult {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来"))
        .expect("合写法");
    let args = args.to_string();
    let script = Script::new([Play::calls(&[("shell", args.as_str())]), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level: if by.is_some() {
                Level::Full
            } else {
                Level::Workspace
            },
            read_only: false,
        },
        attended: false,
        cwd: cwd.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(miyu_sandbox::testkit::built_helper()),
        sandbox_cache: None,
    };
    let lines = Lines {
        venue: match kind {
            Kind::Member => VenueId::parse("local"),
            Kind::Group => VenueId::parse("qq:group:1"),
        }
        .expect("场所合写法"),
        owner_is_admin: false,
        ..Lines::default()
    };
    let handle = home.create_full(&script, &tools, opening, lines).await;
    let mut pushes = watch(&handle).await;
    match by {
        Some(by) => {
            within("回应", handle.command(id("cmd-1"), by, say("hi")))
                .await
                .expect("会话在跑");
        }
        None => {
            ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
        }
    }
    until_turn_ends(&mut pushes).await;
    home.log(handle.id())
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有一条工具结果")
}

/// 结果里给她看的字。
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

/// 跑 `command`，交回结果。
async fn run(home: &Home, kind: Kind, cwd: &Path, command: &str) -> ToolResult {
    shell(
        home,
        kind,
        cwd,
        serde_json::json!({ "command": command, "description": "Test" }),
    )
    .await
}

/// 场地：这种会话的工作目录，里面放好 `own.txt`；别的成员的家目录、数据根、跑核心的用户的家目录、别处各放一份要藏住的。
fn site(home: &Home, kind: Kind) -> (PathBuf, Vec<PathBuf>) {
    let cwd = match kind {
        Kind::Member => home.root.workspace(&alice_account()),
        Kind::Group => home.scratch.0.join("work"),
    };
    std::fs::create_dir_all(&cwd).expect("建得了工作目录");
    std::fs::write(cwd.join("own.txt"), "mine\n").expect("写得进");
    let secret = |dir: PathBuf| -> PathBuf {
        std::fs::create_dir_all(&dir).expect("建得了目录");
        let file = dir.join("secret.txt");
        std::fs::write(&file, "secret\n").expect("写得进");
        file
    };
    let hidden = vec![
        secret(home.root.path().join("home").join("bob").join("workspace")),
        secret(home.root.path().join("system")),
        secret(home.home.clone()),
        secret(home.scratch.0.join("elsewhere")),
    ];
    (cwd, hidden)
}

#[tokio::test]
async fn a_confined_command_reads_only_its_own_places_and_what_programs_need() {
    let helper = miyu_sandbox::testkit::built_helper();
    let probe = miyu_sandbox::probe(&helper, std::time::Duration::from_secs(60)).expect("探得了");
    for kind in [Kind::Member, Kind::Group] {
        let home = Home::outside_temp();
        let (cwd, hidden) = site(&home, kind);
        if !miyu_sandbox::CONFINES_READS {
            let result = run(&home, kind, &cwd, "cat own.txt").await;
            assert_eq!(
                result.human.as_ref().map(|said| said.key.as_str()),
                Some("core/permissions/no-commands"),
                "{kind:?}：关不住读的平台上不跑"
            );
            continue;
        }
        if probe.mechanisms.is_empty() {
            return;
        }
        let result = run(&home, kind, &cwd, "cat own.txt").await;
        let said = text(&result);
        assert_eq!(
            (result.status, said.trim()),
            (ToolStatus::Ok, "mine"),
            "{kind:?}"
        );
        let result = run(
            &home,
            kind,
            &cwd,
            "cat /etc/hosts >/dev/null && git --version && test \"$HOME\" -ef .",
        )
        .await;
        assert_eq!(result.status, ToolStatus::Ok, "{kind:?}：{}", text(&result));
        let mut denied: Vec<String> = hidden
            .iter()
            .map(|file| file.display().to_string())
            .collect();
        denied.extend(
            ["/etc/shells", "/etc/fstab", "/opt"]
                .into_iter()
                .filter(|path| Path::new(path).exists())
                .map(str::to_string),
        );
        for path in denied {
            // 真的去读：Landlock 管的是打开，`test -r`（`access`）照样说读得到。
            let command = format!(
                "if [ -d '{path}' ]; then ls '{path}'; else cat '{path}'; fi >/dev/null 2>&1 && echo READABLE"
            );
            let result = run(&home, kind, &cwd, &command).await;
            assert!(
                !text(&result).contains("READABLE"),
                "{kind:?}：{path} 读不到：{}",
                text(&result)
            );
        }
    }
}

#[tokio::test]
async fn going_outside_the_sandbox_is_refused_without_asking_or_naming_core_paths() {
    for kind in [Kind::Member, Kind::Group] {
        let home = Home::outside_temp();
        let (cwd, _) = site(&home, kind);
        let result = shell(
            &home,
            kind,
            &cwd,
            serde_json::json!({ "command": "true", "description": "Test", "outside_sandbox": true }),
        )
        .await;
        assert_eq!(
            result.human.as_ref().map(|said| said.key.as_str()),
            Some("core/permissions/no-commands"),
            "{kind:?}"
        );
        let said = text(&result);
        for core_path in [
            home.root.path(),
            home.home.as_path(),
            home.scratch.0.as_path(),
        ] {
            assert!(
                !said.contains(&core_path.display().to_string()),
                "{kind:?}：拒的话里没有 {}：{said}",
                core_path.display()
            );
        }
    }
}

/// 群里主人对应表认出的那个账号开口（施工 MB-1，2026-10-24 项目主人问到）：群会话照样关在群的工作区里，完全放开也一样；
/// 读不到别处，要出沙盒的照样拒、不问。群里他仍是外部身份（设计 06 U8），会话的属主是接平台的系统账号。
#[tokio::test]
async fn the_owner_speaking_in_a_group_is_confined_like_everyone() {
    use miyu_kernel::id::ExternalId;
    use miyu_kernel::origin::{By, External};
    let owner = || {
        Some(By::External(External {
            venue: VenueId::parse("qq:group:1").expect("合写法"),
            id: ExternalId::parse("qq:10001").expect("合写法"),
            account: Some(alice_account()),
            role: None,
        }))
    };
    let home = Home::outside_temp();
    let (cwd, hidden) = site(&home, Kind::Group);
    let outside =
        serde_json::json!({ "command": "true", "description": "Test", "outside_sandbox": true });
    let result = shell_as(&home, Kind::Group, &cwd, outside, owner()).await;
    assert_eq!(
        result.human.as_ref().map(|said| said.key.as_str()),
        Some("core/permissions/no-commands"),
        "要出沙盒的拒"
    );
    if !miyu_sandbox::CONFINES_READS {
        return;
    }
    let elsewhere = hidden.last().expect("有别处的").display().to_string();
    let command = format!("cat '{elsewhere}' >/dev/null 2>&1 && echo READABLE; cat own.txt");
    let read = serde_json::json!({ "command": command, "description": "Test" });
    let result = shell_as(&home, Kind::Group, &cwd, read, owner()).await;
    let said = text(&result);
    assert!(
        !said.contains("READABLE") && said.contains("mine"),
        "{said}"
    );
}
