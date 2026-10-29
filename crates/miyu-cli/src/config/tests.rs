//! `miyu config` 印的样子（`config.md`「样子」）：`get` 的值、报错一行、`check` 的合计、`explain` 的几行照显示的宽度对齐，
//! 两种语言；核心报的文件换成真的位置、家目录写成 `~/…`；还没有项目配置时它该在哪。

use std::path::PathBuf;

use serde_json::json;

use super::paths::{Places, planned};
use super::render::{columns, explain, problem, toml, values};
use super::{ConfigCommand, ConfigPlan, tidy};
use crate::ask::Format;
use crate::diagnostic::Level;
use crate::language::Language;

/// 假的家目录：绝对路径，三个平台都是（`/home/me` 在 Windows 上不算绝对路径）。
fn home() -> PathBuf {
    std::env::temp_dir().join("me")
}

fn places() -> Places {
    Places::of(&ConfigPlan {
        command: ConfigCommand::Get {
            keys: Vec::new(),
            format: Format::Text,
        },
        language: Language::Chinese,
        cwd: home().join("work"),
        root: home().join(".miyu"),
        home: Some(home()),
        color: false,
        gray: false,
    })
}

#[test]
fn get_prints_one_bare_value_or_every_key_as_toml() {
    let items = json!({
        "log.level": {"origin": {"layer": "default"}, "value": "info"},
        "permission.start_read_only": {"origin": {"layer": "default"}, "value": false},
        "ui.language": {"origin": {"layer": "default"}, "value": "zh"},
    });
    assert_eq!(
        values(&items, false),
        "log.level = \"info\"\npermission.start_read_only = false\nui.language = \"zh\"\n"
    );
    let one = json!({"ui.language": {"value": "zh"}});
    assert_eq!(values(&one, true), "zh\n", "只写一个键的只印值，字不带引号");
    let switch = json!({"permission.start_read_only": {"value": true}});
    assert_eq!(values(&switch, true), "true\n");
}

#[test]
fn values_are_written_as_toml() {
    assert_eq!(toml(&json!("a\"b")), "\"a\\\"b\"");
    assert_eq!(toml(&json!(3)), "3");
    assert_eq!(toml(&json!(["a", "b"])), "[\"a\", \"b\"]");
}

#[test]
fn a_problem_line_starts_with_path_line_and_column() {
    let found = json!({"code":"unknown_key","column":1,"level":"warning","line":7,
        "message":"没有 ui.langauge 这一项。是不是想写 ui.language？已忽略此行。"});
    let file = "~/.miyu/home/admin/settings.toml";
    assert_eq!(
        problem(file, &found, Language::Chinese).paint(false),
        "~/.miyu/home/admin/settings.toml:7:1 警告：没有 ui.langauge 这一项。是不是想写 ui.language？已忽略此行。\n"
    );
    let english = json!({"level":"error","line":2,"column":9,"message":"Not valid TOML."});
    assert_eq!(
        problem("~/.miyu/config.toml", &english, Language::English).paint(false),
        "~/.miyu/config.toml:2:9 error: Not valid TOML.\n"
    );
    let whole = json!({"level":"error","message":"这份文件超过 1 MiB，不读。"});
    assert_eq!(
        problem("~/a.toml", &whole, Language::Chinese).paint(false),
        "~/a.toml 错误：这份文件超过 1 MiB，不读。\n",
        "整份的问题没有行列"
    );
    assert_eq!(
        problem("~/a.toml", &whole, Language::Chinese).paint(true),
        "~/a.toml \u{1b}[31m错误\u{1b}[0m：这份文件超过 1 MiB，不读。\u{1b}[0m\n",
        "错误红"
    );
    assert!(
        problem(file, &found, Language::English)
            .paint(true)
            .contains("\u{1b}[33mwarning"),
        "警告黄"
    );
}

#[test]
fn the_total_leaves_out_what_there_is_none_of() {
    assert_eq!(Language::Chinese.problems_total(2, 1), "2 处错误，1 处警告");
    assert_eq!(Language::Chinese.problems_total(0, 3), "3 处警告");
    assert_eq!(Language::Chinese.problems_total(0, 0), "没有问题");
    assert_eq!(
        Language::English.problems_total(2, 1),
        "2 errors, 1 warning"
    );
    assert_eq!(Language::English.problems_total(1, 0), "1 error");
    assert_eq!(Language::English.problems_total(0, 0), "no problems");
}

/// `config.get` 带 `all` 的一项和 `config.schema` 的一项：图纸「样子」里 `ui.language` 的例子。
fn language_item() -> (serde_json::Value, serde_json::Value) {
    let item = json!({
        "layers": [
            {"origin": {"file": "config.toml", "layer": "system", "line": 5}, "used": true, "value": "zh"},
            {"origin": {"layer": "default"}, "used": false, "value": "auto"},
        ],
        "origin": {"file": "config.toml", "layer": "system", "line": 5},
        "value": "zh",
    });
    let said = |name: &str, description: &str| json!({"applies": "now", "description": description, "key": "ui.language", "name": name});
    let chinese = said(
        "界面语言",
        "终端、网页、命令行给你看的字用哪种话。auto 跟着终端或浏览器的语言。",
    );
    (item, chinese)
}

#[test]
fn explain_lines_up_the_layers_like_the_blueprint() {
    let (item, said) = language_item();
    let lines: Vec<String> = explain("ui.language", &item, &said, Language::Chinese, &places())
        .iter()
        .map(|line| line.paint(false))
        .collect();
    // 路径照平台的分隔符写（`~/…` 在 Windows 上是 `~\…`），别的一字不差。
    let system = places().shown("config.toml");
    assert_eq!(
        lines.concat(),
        format!(
            "界面语言（ui.language）：终端、网页、命令行给你看的字用哪种话。auto 跟着终端或浏览器的语言。立即生效。\n\
             \x20 \"zh\"    系统配置  {system}:5  ← 生效\n\
             \x20 \"auto\"  默认值\n"
        )
    );
    let english = json!({"applies": "now", "name": "Interface language",
        "description": "The language terminals, the web page and the command line use for you. auto follows the terminal or browser."});
    let lines: Vec<String> = explain("ui.language", &item, &english, Language::English, &places())
        .iter()
        .map(|line| line.paint(false))
        .collect();
    assert_eq!(
        lines.concat(),
        format!(
            "Interface language (ui.language): The language terminals, the web page and the command line use for you. auto follows the terminal or browser. Takes effect immediately.\n\
             \x20 \"zh\"    system config  {system}:5  ← in effect\n\
             \x20 \"auto\"  default\n"
        )
    );
}

#[test]
fn explain_marks_the_environment_and_what_does_not_count() {
    let item = json!({"layers": [
        {"origin": {"layer": "env", "name": "MIYU_LOG"}, "used": true, "value": "debug"},
        {"origin": {"file": "~/src/app/.miyu/config.toml", "layer": "project", "line": 3}, "problem": "not_tightening", "used": false, "value": false},
        {"origin": {"layer": "default"}, "used": false, "value": "info"},
    ]});
    let said = json!({"applies": "now", "description": "说明。", "name": "名字"});
    let lines = explain("log.level", &item, &said, Language::Chinese, &places());
    assert_eq!(
        lines[1].paint(false),
        "  \"debug\"  环境变量 MIYU_LOG  ← 生效，只管这一次启动\n"
    );
    let project = places().shown("~/src/app/.miyu/config.toml");
    assert_eq!(
        lines[2].paint(false),
        format!("  false    项目配置           {project}:3  ← 不算：比下面几层宽\n")
    );
    let painted = lines[2].paint(true);
    assert!(
        painted.starts_with("\u{1b}[90m") && painted.contains("\u{1b}[31m← 不算"),
        "{painted:?}"
    );
    assert_eq!(lines[3].paint(false), "  \"info\"   默认值\n");
}

#[test]
fn files_are_found_and_shown_with_a_tilde() {
    let places = places();
    assert_eq!(
        places.absolute("config.toml"),
        home().join(".miyu").join("config.toml")
    );
    assert_eq!(
        places.absolute("~/src/app/.miyu/config.toml"),
        home()
            .join("src")
            .join("app")
            .join(".miyu")
            .join("config.toml")
    );
    let elsewhere = std::env::temp_dir().join("elsewhere").join("config.toml");
    assert_eq!(
        places.absolute(&elsewhere.to_string_lossy()),
        elsewhere,
        "绝对路径照原样"
    );
    assert_eq!(
        places.shown("config.toml"),
        format!(
            "~{}",
            ["", ".miyu", "config.toml"].join(std::path::MAIN_SEPARATOR_STR)
        )
    );
    assert_eq!(columns("系统配置"), 8);
}

#[test]
fn a_missing_project_config_belongs_at_the_repository_root() {
    let dir = std::env::temp_dir().join(format!("miyu-cli-config-{}", std::process::id()));
    let deep = dir.join("repo").join("src");
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::create_dir_all(dir.join("repo").join(".git")).unwrap();
    assert_eq!(
        planned(&deep),
        dir.join("repo").join(".miyu").join("config.toml")
    );
    std::fs::remove_dir_all(dir.join("repo").join(".git")).unwrap();
    assert_eq!(
        planned(&deep),
        deep.join(".miyu").join("config.toml"),
        "没有仓库的是当前目录"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

// 施工 8-3：改、写、信任的几个子命令印的字，和图纸「给人看的字」「样子」一字不差（两种语言）；编辑器照 `VISUAL`、`EDITOR`
// 挑，经 shell 跑、文件名里有空格也行。

#[test]
fn set_and_unset_lines_match_the_blueprint() {
    let (zh, en) = (Language::Chinese, Language::English);
    assert_eq!(
        zh.saved("ui.language", "\"zh\"", "now"),
        "已设置 ui.language = \"zh\"（立即生效）"
    );
    assert_eq!(
        en.saved("ui.language", "\"zh\"", "now"),
        "set ui.language = \"zh\" (takes effect immediately)"
    );
    assert_eq!(
        en.saved("permission.start_read_only", "true", "new_session"),
        "set permission.start_read_only = true (applies to new sessions)"
    );
    assert_eq!(
        zh.saved("ui.startup", "\"recent\"", "head_start"),
        "已设置 ui.startup = \"recent\"（重启后生效）"
    );
    assert_eq!(
        zh.overridden("log.level", "env", "\"debug\"")
            .lines(zh, Level::Warning),
        ["miyu: 警告：log.level 被环境变量覆盖，使用 \"debug\""]
    );
    assert_eq!(
        en.overridden("log.level", "env", "\"debug\"")
            .lines(en, Level::Warning),
        ["miyu: warning: log.level is overridden by the environment; using \"debug\""]
    );
    assert_eq!(zh.already("\"zh\""), "已经是 \"zh\"，未修改");
    assert_eq!(en.already("\"zh\""), "already \"zh\", nothing changed");
    assert_eq!(
        zh.removed("ui.language", "\"auto\"", "default"),
        "已删除 ui.language，现在是 \"auto\"（默认值）"
    );
    assert_eq!(
        en.removed("ui.language", "\"auto\"", "default"),
        "removed ui.language, now \"auto\" (default)"
    );
    assert_eq!(
        zh.not_there("ui.language", "system"),
        "系统配置中没有 ui.language，未修改"
    );
    assert_eq!(
        en.not_there("log.level", "system"),
        "log.level is not in the system config, nothing changed"
    );
    assert_eq!(
        zh.project_by_hand().lines(zh, Level::Error),
        [
            "miyu: 项目配置只能手动编辑",
            "提示：运行 'miyu config edit --project'",
        ]
    );
}

#[test]
fn edit_and_trust_words_match_the_blueprint() {
    let (zh, en) = (Language::Chinese, Language::English);
    assert_eq!(
        zh.edit_errors(1),
        "有 1 处错误，未保存。按回车继续编辑，输入 q 放弃："
    );
    assert_eq!(
        en.edit_errors(2),
        "2 errors, not saved. Press Enter to keep editing, or type q to give up: "
    );
    assert_eq!(zh.edit_saved(&["now"]), "已保存（立即生效）");
    assert_eq!(
        en.edit_saved(&["now", "new_session"]),
        "saved (takes effect immediately, applies to new sessions)"
    );
    assert_eq!(en.edit_saved(&[]), "saved");
    assert_eq!(
        en.edit_conflict("~/x").lines(en, Level::Error),
        [
            "miyu: the file changed during editing; not saved",
            "hint: the edited copy is kept in ~/x",
        ]
    );
    assert_eq!(
        en.editor_failed(Some(3)).lines(en, Level::Error),
        ["miyu: the editor did not exit cleanly (3); not saved"]
    );
    assert_eq!(
        zh.editor_failed(None).lines(zh, Level::Error),
        ["miyu: 编辑器异常退出（-），未保存"]
    );
    assert_eq!(
        en.would_set("~/src/app/.miyu/config.toml"),
        "~/src/app/.miyu/config.toml would set (a project config can only tighten limits):"
    );
    assert_eq!(
        en.trust_question("~/src/app/.miyu/config.toml"),
        "Trust '~/src/app/.miyu/config.toml'? [y/N] "
    );
    assert_eq!(
        zh.trust_question("~/src/app/.miyu/config.toml"),
        "信任 '~/src/app/.miyu/config.toml'？[y/N] "
    );
    assert_eq!(
        en.trust_answered(true),
        "trusted (will ask again if the file changes)"
    );
    assert_eq!(
        zh.trust_needs_terminal().lines(zh, Level::Error),
        ["miyu: 需要在终端中确认", "提示：加上 '--yes' 或 '--no'"]
    );
    assert_eq!(
        zh.untrusted_project("~/src/app/.miyu/config.toml")
            .lines(zh, Level::Warning),
        [
            "miyu: 警告：项目配置 ~/src/app/.miyu/config.toml 尚未信任，本次没有使用",
            "提示：运行 'miyu config trust' 查看后决定",
        ]
    );
    assert_eq!(
        en.untrusted_project("~/src/app/.miyu/config.toml")
            .lines(en, Level::Warning),
        [
            "miyu: warning: project config ~/src/app/.miyu/config.toml is not trusted; not used this time",
            "hint: run 'miyu config trust' to review it",
        ]
    );
    assert_eq!(
        zh.explain_header("启动时打开", "ui.startup", "说明。", "head_start"),
        "启动时打开（ui.startup）：说明。重启后生效。"
    );
}

#[test]
fn the_editor_is_visual_then_editor_then_the_system_one() {
    use super::console::editor;
    assert_eq!(editor(Some("code --wait"), Some("nano")), "code --wait");
    assert_eq!(editor(Some("  "), Some("nano")), "nano", "空的不算");
    assert_eq!(editor(None, Some("nano")), "nano");
    let fallback = match cfg!(windows) {
        true => "notepad",
        false => "vi",
    };
    assert_eq!(editor(None, None), fallback);
    assert_eq!(editor(Some(""), Some("")), fallback);
}

#[test]
fn the_editor_runs_through_the_shell_with_the_file_as_one_argument() {
    use super::console::editor_command;
    let dir = std::env::temp_dir().join(format!("miyu-cli-editor-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("new text.toml");
    std::fs::write(&source, "a = 1\n").unwrap();
    let target = dir.join("my settings.toml");
    std::fs::write(&target, "").unwrap();
    // 一个「编辑器」：把准备好的字抄进交给它的文件。带参数的命令照 shell 的写法。
    let editor = match cfg!(windows) {
        true => format!("copy /y \"{}\"", source.display()),
        false => format!("cp '{}'", source.display()),
    };
    let status = editor_command(&editor, &target).output().unwrap().status;
    assert!(status.success(), "{status:?}");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "a = 1\n");
    std::fs::remove_dir_all(&dir).unwrap();
}

// 施工 8-3 补：生效时机五种都照蓝图说（以前 `next_turn` 说成「当场生效」），认不出的不说；人敲的键照核心的写法规整，
// 带多余引号的也找得到回应。

#[test]
fn every_applies_value_is_said_like_the_blueprint() {
    let (zh, en) = (Language::Chinese, Language::English);
    let key = "providers.dev.models.m-1.window";
    let said = [
        ("now", "立即生效", "takes effect immediately"),
        ("new_session", "新会话生效", "applies to new sessions"),
        ("head_start", "重启后生效", "takes effect after restart"),
        ("next_turn", "下一轮生效", "takes effect next turn"),
        (
            "restart",
            "重启核心后生效",
            "takes effect after the core restarts",
        ),
    ];
    for (applies, chinese, english) in said {
        assert_eq!(
            zh.saved(key, "4096", applies),
            format!("已设置 {key} = 4096（{chinese}）")
        );
        assert_eq!(
            en.saved(key, "4096", applies),
            format!("set {key} = 4096 ({english})")
        );
        assert_eq!(zh.edit_saved(&[applies]), format!("已保存（{chinese}）"));
        assert_eq!(
            zh.explain_header("窗口", key, "说明。", applies),
            format!("窗口（{key}）：说明。{chinese}。")
        );
        let mut capital = english.chars();
        let first = capital.next().map(|first| first.to_ascii_uppercase());
        assert_eq!(
            en.explain_header("Window", key, "About.", applies),
            format!(
                "Window ({key}): About. {}{}.",
                first.unwrap_or_default(),
                capital.as_str()
            )
        );
    }
    // 认不出的（核心比命令行新）：不说什么时候生效，不瞎说「当场」。
    assert_eq!(
        zh.saved(key, "4096", "someday"),
        format!("已设置 {key} = 4096")
    );
    assert_eq!(
        en.saved(key, "4096", "someday"),
        format!("set {key} = 4096")
    );
    assert_eq!(zh.edit_saved(&["someday"]), "已保存");
    assert_eq!(
        en.edit_saved(&["now", "someday"]),
        "saved (takes effect immediately)"
    );
    assert_eq!(
        zh.explain_header("窗口", key, "说明。", "someday"),
        format!("窗口（{key}）：说明。")
    );
    assert_eq!(
        en.explain_header("Window", key, "About.", "someday"),
        format!("Window ({key}): About.")
    );
}

#[test]
fn a_typed_key_is_tidied_the_way_the_core_writes_it() {
    // 多余的引号去掉；名字里有点的引号留着；拆不开的原样交，由核心报错。
    assert_eq!(
        tidy("providers.dev.models.\"deepseek-v4-flash\".temperature"),
        "providers.dev.models.deepseek-v4-flash.temperature"
    );
    assert_eq!(
        tidy("providers . dev .models.'m-1'.window"),
        "providers.dev.models.m-1.window"
    );
    assert_eq!(
        tidy("providers.dev.models.\"deepseek-v4.1-flash\".window"),
        "providers.dev.models.\"deepseek-v4.1-flash\".window"
    );
    assert_eq!(tidy("ui.language"), "ui.language");
    assert_eq!(tidy("ui..language"), "ui..language");
    assert_eq!(tidy("ui.\"lang"), "ui.\"lang");
}
