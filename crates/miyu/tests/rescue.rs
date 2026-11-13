//! `miyu rescue`（施工 RC-1，`docs/blueprint/cli/rescue.md`）：不经过核心，照 Miyu 的对话模型填好 `LITEMIYU_RESCUE_*`，拉起
//! `litemiyu --rescue`，其余参数原样交过去、退出码照它的；本来设了的变量不覆盖；配置读不了的一样都不填、照常拉起；找不到的
//! 说怎么装、退出码 1。新命令：改之前 `rescue` 是不认识的命令（退出码 2），这些测试本身就是红的证明。
//!
//! 假的 `litemiyu` 是 shell 脚本，只在 Unix 上跑；Windows 上只测帮助和找不到。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::support::{Home, MIYU};
use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;

/// 四个变量：开发机的环境里设了的，测试里先去掉。
const VARS: [&str; 4] = [
    "LITEMIYU_RESCUE_BASE_URL",
    "LITEMIYU_RESCUE_MODEL",
    "LITEMIYU_RESCUE_API",
    "LITEMIYU_RESCUE_API_KEY",
];

/// 照 `program`（主程序，或者链到它的）跑 `args`：数据根是 `home` 的，界面语言 `lang`，`PATH` 只有 `path`。
fn rescue(program: &Path, home: &Home, lang: &str, path: &[PathBuf], args: &[&str]) -> Command {
    let mut command = Command::new(program);
    command
        .args(args)
        .env("MIYU_HOME", home.root.path())
        .envs(crate::support::offline(home.root.path()))
        .env("MIYU_RESOURCES", crate::support::resources())
        .env("PATH", std::env::join_paths(path).expect("连得起来"))
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("XDG_RUNTIME_DIR");
    for name in VARS {
        command.env_remove(name);
    }
    command
}

fn output(mut command: Command) -> Output {
    command.output().expect("跑得起来")
}

/// 数据根里一个空目录：放假的 `litemiyu`，或者当作什么都没有的 `PATH`。
fn dir(home: &Home, name: &str) -> PathBuf {
    let dir = home.dir.join(name);
    std::fs::create_dir_all(&dir).expect("建得了");
    dir
}

#[test]
fn the_help_is_its_own_page_and_the_list_has_it() {
    let home = Home::new();
    let empty = dir(&home, "empty");
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        for args in [
            &["rescue", "--help"][..],
            &["rescue", "-h"],
            &["help", "rescue"],
        ] {
            let said = output(rescue(
                Path::new(MIYU),
                &home,
                lang,
                std::slice::from_ref(&empty),
                args,
            ));
            assert!(said.status.success(), "{args:?}：{said:?}");
            assert_eq!(
                String::from_utf8_lossy(&said.stdout),
                page(language, Page::Rescue),
                "{lang} {args:?}"
            );
        }
        assert!(page(language, Page::Miyu).contains("\n  rescue [<"));
    }
}

#[test]
fn a_missing_litemiyu_says_how_to_get_it_and_exits_1() {
    let home = Home::new();
    let empty = dir(&home, "empty");
    let said = output(rescue(
        Path::new(MIYU),
        &home,
        "zh_CN.UTF-8",
        std::slice::from_ref(&empty),
        &["rescue"],
    ));
    assert_eq!(said.status.code(), Some(1), "{said:?}");
    let second = match cfg!(target_os = "linux") {
        true => {
            "提示：安装：Arch Linux 用 AUR（paru -S litemiyu 或 yay -S litemiyu）；其他发行版从 https://github.com/miyu-agent/litemiyu/releases 下载"
        }
        false => "提示：暂只支持 Linux",
    };
    assert_eq!(
        String::from_utf8_lossy(&said.stderr),
        format!("miyu: 未找到 litemiyu\n{second}\n")
    );
    assert!(said.stdout.is_empty());

    let said = output(rescue(Path::new(MIYU), &home, "C", &[empty], &["rescue"]));
    assert_eq!(said.status.code(), Some(1), "{said:?}");
    assert!(
        String::from_utf8_lossy(&said.stderr).starts_with("miyu: litemiyu not found\nhint: "),
        "{said:?}"
    );
    assert!(home.core_log().is_empty(), "核心没起来过");
}

/// 假的 `litemiyu`：把收到的参数、四个变量写进同一个目录的 `seen.txt`，退出码 7。经 `sh` 写：这个进程不拿着它的写端，别的
/// 测试这时起进程，也不会让它执行不了（ETXTBSY，同 `miyu-core` 的假助手）。
#[cfg(unix)]
fn fake(dir: &Path) -> PathBuf {
    let seen = dir.join("seen.txt");
    let vars: String = VARS
        .iter()
        .map(|name| format!("printf '{name}=%s\\n' \"${{{name}-<unset>}}\"\n"))
        .collect();
    let script = format!(
        "#!/bin/sh\n{{\nfor a in \"$@\"; do printf 'arg=%s\\n' \"$a\"; done\n{vars}}} > '{}'\nexit 7\n",
        seen.display()
    );
    let program = dir.join("litemiyu");
    let status = Command::new("/bin/sh")
        .args(["-c", r#"printf '%s' "$2" > "$1" && chmod 755 "$1""#, "sh"])
        .arg(&program)
        .arg(script)
        .status()
        .expect("起得来");
    assert!(status.success());
    seen
}

/// 假的 `litemiyu` 记下的：一行一样。
#[cfg(unix)]
fn seen(file: &Path) -> Vec<String> {
    std::fs::read_to_string(file)
        .unwrap_or_else(|error| panic!("{} 没跑：{error}", file.display()))
        .lines()
        .map(str::to_string)
        .collect()
}

/// 配好一家手写的 Anthropic 和它的 key。
#[cfg(unix)]
fn configured(home: &Home) {
    home.system_config(
        "[providers.lab]\ndriver = \"anthropic\"\nbase_url = \"http://127.0.0.1:9/v1\"\nkey = { secret = \"lab\" }\n\n[models]\nchat = \"lab/claude-x\"\n",
    );
    std::fs::write(
        home.root.path().join("etc/private/secrets.toml"),
        "lab = \"sk-test\"\n",
    )
    .expect("写得进");
}

#[cfg(unix)]
#[test]
fn the_chat_model_goes_in_the_environment_and_the_arguments_go_as_they_are() {
    let home = Home::new();
    configured(&home);
    let bin = dir(&home, "bin");
    let seen_file = fake(&bin);
    let said = output(rescue(
        Path::new(MIYU),
        &home,
        "C",
        &[bin],
        &["rescue", "a", "--model=x", "b c", "-h"],
    ));
    assert_eq!(said.status.code(), Some(7), "退出码照它的：{said:?}");
    assert!(said.stderr.is_empty(), "{said:?}");
    assert_eq!(
        seen(&seen_file),
        [
            "arg=--rescue",
            "arg=a",
            "arg=--model=x",
            "arg=b c",
            "arg=-h",
            "LITEMIYU_RESCUE_BASE_URL=http://127.0.0.1:9/v1",
            "LITEMIYU_RESCUE_MODEL=claude-x",
            "LITEMIYU_RESCUE_API=anthropic",
            "LITEMIYU_RESCUE_API_KEY=sk-test",
        ]
    );
    assert!(home.core_log().is_empty(), "核心没起来过");
}

#[cfg(unix)]
#[test]
fn variables_already_set_are_kept() {
    let home = Home::new();
    configured(&home);
    let bin = dir(&home, "bin");
    let seen_file = fake(&bin);
    let mut command = rescue(Path::new(MIYU), &home, "C", &[bin], &["rescue"]);
    command
        .env("LITEMIYU_RESCUE_MODEL", "mine")
        .env("LITEMIYU_RESCUE_API_KEY", "sk-own");
    let said = output(command);
    assert_eq!(said.status.code(), Some(7), "{said:?}");
    assert_eq!(
        seen(&seen_file)[1..],
        [
            "LITEMIYU_RESCUE_BASE_URL=http://127.0.0.1:9/v1",
            "LITEMIYU_RESCUE_MODEL=mine",
            "LITEMIYU_RESCUE_API=anthropic",
            "LITEMIYU_RESCUE_API_KEY=sk-own",
        ]
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_config_fills_nothing_and_still_launches() {
    let home = Home::new();
    let bin = dir(&home, "bin");
    let seen_file = fake(&bin);
    let unset: Vec<String> = VARS.iter().map(|name| format!("{name}=<unset>")).collect();
    let cases = [
        ("[models\n", "config.toml: line 1: "),
        ("", "no model configured: set models.chat"),
    ];
    for (config, why) in cases {
        home.system_config(config);
        let said = output(rescue(
            Path::new(MIYU),
            &home,
            "zh_CN.UTF-8",
            std::slice::from_ref(&bin),
            &["rescue"],
        ));
        assert_eq!(said.status.code(), Some(7), "照常拉起：{said:?}");
        let stderr = String::from_utf8_lossy(&said.stderr);
        assert!(
            stderr.starts_with("miyu: 警告：读不了 Miyu 的配置：")
                && stderr.contains(why)
                && stderr.ends_with("，交给 litemiyu 自己找模型\n")
                && stderr.lines().count() == 1,
            "{stderr}"
        );
        assert_eq!(seen(&seen_file)[1..], unset[..], "{config:?}");
    }
}

/// 对话模型走扩展驱动（没装那个包也照写法认）：LiteMiyu 认不了，一个变量都不填，警告一行，照常拉起（core1 2026-11-13 定）。
#[cfg(unix)]
#[test]
fn an_extension_driver_fills_nothing_and_still_launches() {
    let home = Home::new();
    home.system_config(
        "[providers.ext]\ndriver = \"pi/ag\"\nbase_url = \"http://127.0.0.1:9/v1\"\n\n[models]\nchat = \"ext/m\"\n",
    );
    let bin = dir(&home, "bin");
    let seen_file = fake(&bin);
    let said = output(rescue(
        Path::new(MIYU),
        &home,
        "zh_CN.UTF-8",
        std::slice::from_ref(&bin),
        &["rescue"],
    ));
    assert_eq!(said.status.code(), Some(7), "照常拉起：{said:?}");
    assert_eq!(
        String::from_utf8_lossy(&said.stderr),
        "miyu: 警告：对话模型用的是扩展驱动 pi/ag，litemiyu 认不了，交给它自己找模型\n"
    );
    let unset: Vec<String> = VARS.iter().map(|name| format!("{name}=<unset>")).collect();
    assert_eq!(seen(&seen_file)[1..], unset[..]);
}

/// AUR 的样子：敲的 `miyu` 是链接，它旁边的 `litemiyu` 先于 `PATH` 里的。
#[cfg(unix)]
#[test]
fn beside_the_typed_link_comes_before_path() {
    let home = Home::new();
    let typed = dir(&home, "typed");
    let link = typed.join("miyu");
    std::os::unix::fs::symlink(MIYU, &link).expect("连得上");
    let beside = fake(&typed);
    let path = dir(&home, "path");
    let on_path = fake(&path);
    let said = output(rescue(&link, &home, "C", &[path], &["rescue"]));
    assert_eq!(said.status.code(), Some(7), "{said:?}");
    assert_eq!(seen(&beside)[0], "arg=--rescue");
    assert!(!on_path.exists(), "PATH 里的没跑");
}
