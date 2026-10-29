//! `miyu config set`、`unset`、`edit`、`trust`（施工 8-3，`docs/blueprint/config.md`「命令行」第十条、「样子」）：在进程里
//! 起一个核心，在真的套接字上走一遍。人那一头是照剧本回的：在不在终端里、敲什么、编辑器改成什么。
//!
//! 印的那一行照「样子」一字不差；`edit` 改字、不改、改错再改好、放弃、编辑器出错、存的时候别处改过了、项目配置由命令行
//! 自己写；`trust` 列出会改哪几项、`--yes`、`--no`、问、不在终端里、本来就信任着、看的时候又变了。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::support::configuring::{Edit, Fake, at_terminal};
use crate::support::outside::Outside;
use crate::support::{Asked, Home};
use miyu_cli::language::Language;
use miyu_cli::{ConfigCommand, ConfigPlan};
use miyu_session::testkit::Script;

fn home() -> Home {
    Home::new(Arc::new(Script::new([])))
}

/// 在 `cwd` 里办 `command`，中文。
async fn run(home: &Home, cwd: &Path, command: ConfigCommand, console: &mut Fake) -> Asked {
    let plan = ConfigPlan {
        command,
        language: Language::Chinese,
        cwd: cwd.to_path_buf(),
        root: home.root.path().to_path_buf(),
        home: None,
        color: false,
        gray: false,
    };
    home.config(&plan, console).await
}

fn set(key: &str, value: &str) -> ConfigCommand {
    ConfigCommand::Set {
        key: key.to_string(),
        value: value.to_string(),
        project: false,
    }
}

fn unset(key: &str) -> ConfigCommand {
    ConfigCommand::Unset {
        key: key.to_string(),
    }
}

fn edit(project: bool) -> ConfigCommand {
    ConfigCommand::Edit { project }
}

fn trust(yes: bool, no: bool) -> ConfigCommand {
    ConfigCommand::Trust { yes, no }
}

/// 系统配置在哪。
fn config_file(home: &Home) -> PathBuf {
    home.root.path().join("config.toml")
}

/// 系统配置旁边还有没有编辑时的副本。
fn copies_left(home: &Home) -> Vec<String> {
    std::fs::read_dir(home.root.path().join("system"))
        .map(|entries| {
            entries
                .map(|entry| {
                    entry
                        .expect("在")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .filter(|name| name.contains(".edit-"))
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn set_and_unset_say_what_happened_in_one_gray_line() {
    let home = home();
    let cwd = std::env::temp_dir();
    let mut none = Fake::default();
    let cases = [
        (
            set("ui.language", "zh"),
            0,
            "已设置 ui.language = \"zh\"（立即生效）\n",
        ),
        (set("ui.language", "\"zh\""), 0, "已经是 \"zh\"，未修改\n"),
        (
            set("permission.start_read_only", "true"),
            0,
            "已设置 permission.start_read_only = true（新会话生效）\n",
        ),
        (
            set("ui.language", "en"),
            0,
            "已设置 ui.language = \"en\"（立即生效）\n",
        ),
        // 定了英文：握手以后照英文说。
        (
            unset("ui.language"),
            0,
            "removed ui.language, now \"auto\" (default)\n",
        ),
        // 删掉以后跟着系统语言（中文）。
        (
            unset("ui.language"),
            0,
            "系统配置中没有 ui.language，未修改\n",
        ),
        (
            set("ui.language", "zh"),
            0,
            "已设置 ui.language = \"zh\"（立即生效）\n",
        ),
    ];
    for (command, code, said) in cases {
        let asked = run(&home, &cwd, command.clone(), &mut none).await;
        assert_eq!(
            (asked.code, asked.err.as_str()),
            (code, said),
            "{command:?}"
        );
        assert_eq!(asked.out, "");
    }
    let wrong = run(&home, &cwd, set("ui.language", "cn"), &mut none).await;
    assert_eq!(wrong.code, 1);
    assert_eq!(
        wrong.err,
        "miyu: ui.language 只能是 auto、zh、en 或 ja，写的是 cn。改成其中一个，例如 ui.language = \"auto\"\n"
    );
    let unknown = run(&home, &cwd, set("ui.langauge", "zh"), &mut none).await;
    assert_eq!(unknown.code, 1);
    assert!(unknown.err.contains("ui.language"), "{}", unknown.err);
    let project = ConfigCommand::Set {
        key: "permission.start_read_only".to_string(),
        value: "true".to_string(),
        project: true,
    };
    let refused = run(&home, &cwd, project, &mut none).await;
    assert_eq!(
        (refused.code, refused.err.as_str()),
        (
            2,
            "miyu: 项目配置只能手动编辑\n提示：运行 'miyu config edit --project'\n"
        )
    );
}

#[tokio::test]
async fn edit_asks_again_until_it_checks_out_and_keeps_what_was_typed() {
    let home = home();
    let cwd = std::env::temp_dir();
    std::fs::create_dir_all(config_file(&home).parent().expect("有上一级")).expect("建得了");
    std::fs::write(config_file(&home), "[ui]\nlanguage = \"zh\"\n").expect("写得进");
    let wrong = "[ui]\nlanguage = \"cn\"\n";
    let fixed = "# 改好了\n[ui]\nlanguage = \"ja\"\nlangauge = 1\n";
    let mut console = at_terminal(&[""], vec![Edit::Write(wrong), Edit::Write(fixed)]);
    let asked = run(&home, &cwd, edit(false), &mut console).await;
    let shown = config_file(&home).to_string_lossy().into_owned();
    assert_eq!(
        asked.err,
        format!(
            "{shown}:2:12 错误：ui.language 只能是 auto、zh、en 或 ja，写的是 \"cn\"。改成其中一个，例如 ui.language = \"auto\"。暂用 \"auto\"（默认值）。\n\
             有 1 处错误，未保存。按回车继续编辑，输入 q 放弃：\
             {shown}:4:1 警告：没有 ui.langauge 这一项。是不是想写 ui.language？已忽略此行。\n\
             已保存（立即生效）\n"
        )
    );
    assert_eq!(asked.code, 0);
    assert_eq!(
        std::fs::read_to_string(config_file(&home)).expect("在"),
        fixed,
        "只有警告的照样存"
    );
    let [(first, before), (again, kept)] = console.opened.as_slice() else {
        panic!("开了两次：{:?}", console.opened);
    };
    assert_eq!(before, "[ui]\nlanguage = \"zh\"\n", "副本是现在的字");
    assert_eq!(
        (again, kept.as_str()),
        (first, wrong),
        "回车再打开，改的还在"
    );
    assert_eq!(
        first.parent(),
        config_file(&home).parent(),
        "副本在原文件旁边"
    );
    let name = first
        .file_name()
        .expect("有名字")
        .to_string_lossy()
        .into_owned();
    assert!(
        name.starts_with(".config.toml.edit-") && name.ends_with(".toml"),
        "{name}"
    );
    assert_eq!(copies_left(&home), Vec::<String>::new(), "存好了删掉副本");
}

#[tokio::test]
async fn edit_leaves_the_file_alone_when_nothing_is_saved() {
    let home = home();
    let cwd = std::env::temp_dir();
    let bad = "[ui]\nlanguage = \"cn\"\n";
    let cases: [(Fake, u8, &str); 4] = [
        (at_terminal(&[], vec![Edit::Keep]), 0, "未修改\n"),
        (
            at_terminal(&["q"], vec![Edit::Write(bad)]),
            1,
            "已放弃，文件未修改\n",
        ),
        (
            at_terminal(&[], vec![Edit::Write(bad)]),
            1,
            "已放弃，文件未修改\n",
        ),
        (
            at_terminal(&[], vec![Edit::Fail(3)]),
            1,
            "miyu: 编辑器异常退出（3），未保存\n",
        ),
    ];
    for (mut console, code, ends) in cases {
        let asked = run(&home, &cwd, edit(false), &mut console).await;
        assert_eq!(asked.code, code, "{}", asked.err);
        assert!(asked.err.ends_with(ends), "{}", asked.err);
        assert!(!config_file(&home).exists(), "文件没动");
        // 系统配置旁边核心自己的东西（人格目录这些）不算编辑留下的。
        let account = config_file(&home).parent().expect("有上一级").to_path_buf();
        let left: Vec<String> = std::fs::read_dir(&account)
            .map(|entries| {
                entries
                    .map(|entry| {
                        entry
                            .expect("读得了")
                            .file_name()
                            .to_string_lossy()
                            .into_owned()
                    })
                    .filter(|name| {
                        !["packages", "personas", "run", "state", ".miyu-root"]
                            .contains(&name.as_str())
                    })
                    .collect()
            })
            .unwrap_or_default();
        assert!(left.is_empty(), "为副本新建的东西也删掉：{left:?}");
    }
    let mut elsewhere = Fake::default();
    let asked = run(&home, &cwd, edit(false), &mut elsewhere).await;
    assert_eq!(
        (asked.code, asked.err.as_str()),
        (2, "miyu: 'miyu config edit' 需要在终端中运行\n"),
        "不在终端里"
    );
}

#[tokio::test]
async fn edit_keeps_the_copy_when_the_file_changed_meanwhile() {
    let home = home();
    let cwd = std::env::temp_dir();
    std::fs::create_dir_all(config_file(&home).parent().expect("有上一级")).expect("建得了");
    std::fs::write(config_file(&home), "[ui]\nlanguage = \"zh\"\n").expect("写得进");
    let mine = "[ui]\nlanguage = \"ja\"\n";
    let theirs = "[ui]\nlanguage = \"zh\"\n# 别处改的\n";
    let mut console = at_terminal(&[], vec![Edit::Meanwhile(config_file(&home), theirs, mine)]);
    let asked = run(&home, &cwd, edit(false), &mut console).await;
    let copy = &console.opened[0].0;
    assert_eq!(asked.code, 1);
    assert_eq!(
        asked.err,
        format!(
            "miyu: 编辑期间文件已被修改，未保存\n提示：修改内容保留在 {}\n",
            copy.to_string_lossy()
        )
    );
    assert_eq!(
        std::fs::read_to_string(config_file(&home)).expect("在"),
        theirs
    );
    assert_eq!(std::fs::read_to_string(copy).expect("副本留着"), mine);
}

/// 一个仓库（有 `.git`），交回它真实的位置。
fn repository(outside: &Outside) -> PathBuf {
    let repo = outside.0.join("app");
    std::fs::create_dir_all(repo.join(".git")).expect("建得了");
    std::fs::create_dir_all(repo.join("src")).expect("建得了");
    std::fs::canonicalize(&repo).expect("在")
}

/// 核心报的项目配置的写法：真实的位置，Windows 上去掉开头的 `\\?\`。
fn shown(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

#[tokio::test]
async fn a_project_config_is_edited_and_saved_by_the_command_line_then_trusted() {
    let home = home();
    let outside = Outside::new();
    let repo = repository(&outside);
    let cwd = repo.join("src");
    let none = run(&home, &cwd, trust(false, false), &mut Fake::default()).await;
    assert_eq!(
        (none.code, none.err.as_str()),
        (1, "miyu: 当前目录没有项目配置\n")
    );

    let written = "[permission]\nstart_read_only = true\n\n[ui]\nlanguage = \"zh\"\n";
    let mut console = at_terminal(
        &[""],
        vec![
            Edit::Write(written),
            Edit::Write("[permission]\nstart_read_only = true\n"),
        ],
    );
    let edited = run(&home, &cwd, edit(true), &mut console).await;
    assert_eq!(edited.code, 0, "{}", edited.err);
    assert!(
        edited.err.contains("ui.language 只能写在系统配置里"),
        "{}",
        edited.err
    );
    assert!(edited.err.ends_with("已保存\n"), "{}", edited.err);
    let file = repo.join(".miyu").join("config.toml");
    assert_eq!(
        std::fs::read_to_string(&file).expect("在仓库的根新建了"),
        "[permission]\nstart_read_only = true\n"
    );

    let listed = run(&home, &cwd, trust(false, false), &mut Fake::default()).await;
    let header = format!(
        "{} 会修改以下设置（项目配置只能收紧限制）：\n  permission.start_read_only = true  新会话开局只读\n",
        shown(&file)
    );
    assert_eq!(listed.out, header);
    assert_eq!(
        (listed.code, listed.err.as_str()),
        (2, "miyu: 需要在终端中确认\n提示：加上 '--yes' 或 '--no'\n"),
        "不在终端里又没写 --yes、--no"
    );
    let asked = run(
        &home,
        &cwd,
        trust(false, false),
        &mut at_terminal(&["Yes"], Vec::new()),
    )
    .await;
    assert_eq!(asked.out, header);
    assert_eq!(
        (asked.code, asked.err),
        (
            0,
            format!(
                "信任 '{}'？[y/N] 已信任（文件变化后会再次询问）\n",
                shown(&file)
            )
        )
    );
    let again = run(&home, &cwd, trust(false, false), &mut Fake::default()).await;
    assert_eq!((again.code, again.err.as_str()), (0, "已经信任，未修改\n"));
    let no = run(&home, &cwd, trust(false, true), &mut Fake::default()).await;
    assert_eq!(
        (no.code, no.err.as_str()),
        (0, "已拒绝，不会使用（文件变化后会再次询问）\n")
    );
    let enter = run(
        &home,
        &cwd,
        trust(false, false),
        &mut at_terminal(&[""], Vec::new()),
    )
    .await;
    assert!(
        enter
            .err
            .ends_with("已拒绝，不会使用（文件变化后会再次询问）\n"),
        "直接回车是不信任"
    );
    let yes = run(&home, &cwd, trust(true, false), &mut Fake::default()).await;
    assert_eq!(
        (yes.code, yes.err.as_str()),
        (0, "已信任（文件变化后会再次询问）\n")
    );
}

#[tokio::test]
async fn trust_lists_problems_and_refuses_a_version_that_changed_while_looking() {
    let home = home();
    let outside = Outside::new();
    let repo = repository(&outside);
    let file = repo.join(".miyu").join("config.toml");
    std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了");
    std::fs::write(
        &file,
        "[permission]\nstart_read_only = true\n[ui]\nlanguage = \"zh\"\n",
    )
    .expect("写得进");
    let mut console = at_terminal(&["y"], Vec::new());
    console.meanwhile = Some((file.clone(), "[permission]\nstart_read_only = true\n"));
    let asked = run(&home, &repo, trust(false, false), &mut console).await;
    assert_eq!(
        asked.out,
        format!(
            "{0} 会修改以下设置（项目配置只能收紧限制）：\n  permission.start_read_only = true  新会话开局只读\n\
             {0}:4:12 错误：ui.language 只能写在系统配置里，写在项目配置里不算。挪到系统配置里去。\n",
            shown(&file)
        ),
        "不能写的那一项不列，照报错一行印在后面"
    );
    assert_eq!(asked.code, 1);
    assert!(
        asked
            .err
            .ends_with("miyu: 查看期间文件已被修改\n提示：重新运行 'miyu config trust'\n"),
        "{}",
        asked.err
    );
    assert!(
        !home
            .root
            .path()
            .join("home")
            .join("admin")
            .join("trust.toml")
            .exists()
    );
}
