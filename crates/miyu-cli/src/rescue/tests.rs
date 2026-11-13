//! `miyu rescue`（施工 RC-1）：找 `litemiyu` 的先后，填哪几个环境变量、不覆盖本来有的，算不出模型时一样都不填。都是新命令，
//! 改之前没有这几个函数，测试本身就是红的证明。

use std::cell::Cell;
use std::ffi::OsString;
use std::path::PathBuf;

use miyu_config::secret::Secret;
use miyu_models::direct::Direct;
use miyu_models::provider::{Driver, DriverRef};

use super::find::{Places, find};
use super::*;

/// 测试的临时目录，用完删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("miyu-cli-rescue-{name}-{}", std::process::id()));
        if dir.exists() {
            miyu_store::testkit::remove_tree(&dir);
        }
        std::fs::create_dir_all(&dir).expect("建得了");
        Scratch(dir)
    }

    /// 放一个文件（内容随便：只看是不是文件），交回它的路径。
    fn put(&self, file: &str) -> PathBuf {
        let path = self.0.join(file);
        std::fs::create_dir_all(path.parent().expect("有上一级")).expect("建得了");
        std::fs::write(&path, "x").expect("写得进");
        path
    }

    fn at(&self, file: &str) -> PathBuf {
        self.0.join(file)
    }

    /// 几个目录连成 `PATH` 的写法。
    fn path(&self, dirs: &[&str]) -> OsString {
        std::env::join_paths(dirs.iter().map(|dir| self.at(dir))).expect("连得起来")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        miyu_store::testkit::remove_tree(&self.0);
    }
}

#[test]
fn beside_the_typed_path_then_beside_the_real_one_then_path() {
    let scratch = Scratch::new("order");
    let typed = scratch.put("typed/miyu");
    let real = scratch.put("real/miyu");
    let places = Places {
        typed: Some(typed),
        exe: Some(real),
        path: Some(scratch.path(&["path"])),
        cwd: None,
    };
    assert_eq!(find(&places, false), None, "哪里都没有");
    let in_path = scratch.put("path/litemiyu");
    assert_eq!(find(&places, false), Some(in_path.clone()));
    // 真实位置旁边的那一个照解了链接的路径交回（`find` 先把 `exe` 解成真实路径）：macOS 的临时目录在 `/private` 下面，Windows
    // 交回 `\\?\` 打头的长路径，和这里拼的字面不一样，两边都解了再比。
    let beside_real = scratch.put("real/litemiyu");
    assert_eq!(
        resolved(find(&places, false)),
        resolved(Some(beside_real.clone()))
    );
    let beside_typed = scratch.put("typed/litemiyu");
    assert_eq!(find(&places, false), Some(beside_typed.clone()));
    // 是目录的不算。
    std::fs::remove_file(&beside_typed).expect("删得了");
    std::fs::create_dir(&beside_typed).expect("建得了");
    assert_eq!(resolved(find(&places, false)), resolved(Some(beside_real)));
}

/// 解了链接的路径：比「真实位置旁边」的那一个用。
fn resolved(path: Option<PathBuf>) -> Option<PathBuf> {
    path.map(|path| std::fs::canonicalize(path).expect("在"))
}

#[test]
fn a_bare_name_is_found_on_path_where_it_was_typed() {
    // 只敲了 `miyu`：照 `PATH` 找到被敲的那一个（`b/`），它旁边的先于 `PATH` 里排在前面的。
    let scratch = Scratch::new("bare");
    scratch.put("a/litemiyu");
    scratch.put("b/miyu");
    let beside = scratch.put("b/litemiyu");
    let places = Places {
        typed: Some(PathBuf::from("miyu")),
        exe: None,
        path: Some(scratch.path(&["a", "b"])),
        cwd: None,
    };
    assert_eq!(find(&places, false), Some(beside));
}

#[test]
fn a_relative_typed_path_follows_the_current_directory() {
    let scratch = Scratch::new("relative");
    let beside = scratch.put("bin/litemiyu");
    let places = Places {
        typed: Some(PathBuf::from("bin/miyu")),
        cwd: Some(scratch.0.clone()),
        ..Places::default()
    };
    assert_eq!(find(&places, false), Some(beside));
}

#[test]
fn relative_directories_on_path_are_not_trusted() {
    let scratch = Scratch::new("untrusted");
    scratch.put("here/litemiyu");
    let places = Places {
        path: Some(OsString::from("here")),
        cwd: Some(scratch.0.clone()),
        ..Places::default()
    };
    assert_eq!(find(&places, false), None);
}

#[test]
fn windows_looks_for_the_exe() {
    let scratch = Scratch::new("windows");
    scratch.put("bin/litemiyu");
    scratch.put("bin/miyu.exe");
    let places = Places {
        typed: Some(PathBuf::from("miyu")),
        path: Some(scratch.path(&["bin"])),
        ..Places::default()
    };
    assert_eq!(find(&places, true), None, "没有 .exe 的不算");
    let exe = scratch.put("bin/litemiyu.exe");
    assert_eq!(find(&places, true), Some(exe));
}

/// AUR 的样子：`/usr/bin/miyu` 链到 `/usr/lib/miyu/miyu`。
#[cfg(unix)]
#[test]
fn a_linked_miyu_looks_beside_the_link_then_beside_the_target() {
    let scratch = Scratch::new("linked");
    let real = scratch.put("lib/miyu");
    std::fs::create_dir_all(scratch.at("bin")).expect("建得了");
    let link = scratch.at("bin/miyu");
    std::os::unix::fs::symlink(&real, &link).expect("连得上");
    let places = Places {
        typed: Some(link.clone()),
        exe: Some(link),
        ..Places::default()
    };
    let beside_target = scratch.put("lib/litemiyu");
    assert_eq!(
        find(&places, false).map(|path| std::fs::canonicalize(path).expect("在")),
        Some(std::fs::canonicalize(beside_target).expect("在"))
    );
    let beside_link = scratch.put("bin/litemiyu");
    assert_eq!(find(&places, false), Some(beside_link));
}

/// 一个直接连得上的模型。
fn direct(driver: Driver, key: Option<&str>) -> Direct {
    Direct {
        base_url: "https://api.example.invalid/v1".to_string(),
        model: "m-1".to_string(),
        driver,
        key: key.map(|key| Secret::new(key).expect("合写法")),
    }
}

#[test]
fn every_variable_is_filled_from_the_chat_model() {
    let (vars, why) = filled(&|_| false, || Ok(direct(Driver::Anthropic, Some("sk-x"))));
    assert_eq!(why, None);
    assert_eq!(
        vars,
        [
            (BASE_URL, "https://api.example.invalid/v1".to_string()),
            (MODEL, "m-1".to_string()),
            (API, "anthropic".to_string()),
            (API_KEY, "sk-x".to_string()),
        ]
    );
}

#[test]
fn drivers_map_to_the_same_names_and_an_extension_fills_nothing() {
    assert_eq!(api(&Driver::OpenAiChat), Some("openai-chat"));
    assert_eq!(api(&Driver::Anthropic), Some("anthropic"));
    assert_eq!(api(&Driver::OpenAiResponses), Some("openai-responses"));
    let extension = Driver::Extension(DriverRef {
        package: "pi".to_string(),
        name: "ag".to_string(),
    });
    assert_eq!(api(&extension), None);
    // 扩展的驱动：一个都不填，交回驱动的写法（core1 2026-11-13 定）。
    let (vars, unfilled) = filled(&|_| false, || Ok(direct(extension, Some("sk-x"))));
    assert!(vars.is_empty(), "{vars:?}");
    assert_eq!(unfilled, Some(Unfilled::Extension("pi/ag".to_string())));
    // 没有 key 的：只不填 key。
    let (vars, _) = filled(&|_| false, || Ok(direct(Driver::OpenAiChat, None)));
    let names: Vec<&str> = vars.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, [BASE_URL, MODEL, API]);
}

/// 扩展驱动的警告：两种语言。
#[test]
fn an_extension_driver_is_warned_about() {
    use crate::diagnostic::Level;
    use crate::language::Language;
    let line = |language: Language| {
        language
            .rescue_extension("pi/ag")
            .lines(language, Level::Warning)
            .join("\n")
    };
    assert_eq!(
        line(Language::Chinese),
        "miyu: 警告：对话模型用的是扩展驱动 pi/ag，litemiyu 认不了，交给它自己找模型"
    );
    assert_eq!(
        line(Language::English),
        "miyu: warning: the chat model uses the extension driver pi/ag, which litemiyu cannot use; litemiyu will find a model itself"
    );
}

#[test]
fn what_is_already_set_is_kept() {
    let (vars, why) = filled(&|name| name == MODEL || name == API_KEY, || {
        Ok(direct(Driver::OpenAiChat, Some("sk-x")))
    });
    assert_eq!(why, None);
    assert_eq!(
        vars,
        [
            (BASE_URL, "https://api.example.invalid/v1".to_string()),
            (API, "openai-chat".to_string()),
        ]
    );
    // 四个都有的：不读配置。
    let asked = Cell::new(false);
    let (vars, why) = filled(&|_| true, || {
        asked.set(true);
        Err("not asked".to_string())
    });
    assert!(vars.is_empty() && why.is_none() && !asked.get());
}

#[test]
fn nothing_is_filled_when_the_model_cannot_be_read() {
    let (vars, why) = filled(&|_| false, || Err("config.toml: line 1: bad".to_string()));
    assert!(vars.is_empty());
    assert_eq!(
        why,
        Some(Unfilled::Unreadable("config.toml: line 1: bad".to_string()))
    );
}

/// 找不到时说的：两种语言，Linux 说怎么装，别的说暂只支持 Linux。
#[test]
fn a_missing_litemiyu_says_how_to_install_it() {
    use crate::diagnostic::Level;
    use crate::language::Language;
    let lines = |language: Language, linux: bool| {
        language
            .rescue_missing(linux)
            .lines(language, Level::Error)
            .join("\n")
    };
    assert_eq!(
        lines(Language::Chinese, true),
        "miyu: 未找到 litemiyu\n提示：安装：Arch Linux 用 AUR（paru -S litemiyu 或 yay -S litemiyu）；其他发行版从 https://github.com/miyu-agent/litemiyu/releases 下载"
    );
    assert_eq!(
        lines(Language::Chinese, false),
        "miyu: 未找到 litemiyu\n提示：暂只支持 Linux"
    );
    assert_eq!(
        lines(Language::English, true),
        "miyu: litemiyu not found\nhint: to install it, use the AUR on Arch Linux (paru -S litemiyu or yay -S litemiyu); on other distributions, download it from https://github.com/miyu-agent/litemiyu/releases"
    );
    assert_eq!(
        lines(Language::English, false),
        "miyu: litemiyu not found\nhint: only Linux is supported for now"
    );
}
