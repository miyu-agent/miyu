//! 软件包清单在哪、两层怎么认（施工 9-1 上）：出厂的、数据根里的，照编号排；同一个编号认出厂的；子命令名撞了的
//! 后读到的那一份报错；读不了的、不是 `.toml` 的、编号不合写法的。

use std::fs;

use miyu_config::package::{Code, PackageKind};

use super::*;
use crate::env::{Env, Platform};
use crate::test_support::Scratch;

/// 一个临时的资源目录和数据根：`res/`、`data/`。
struct Places {
    scratch: Scratch,
    packages: Packages,
}

impl Places {
    fn new() -> Places {
        let scratch = Scratch::new();
        let resources = ResourceRoot::at(scratch.path().join("res"));
        let env = Env {
            platform: Platform::current(),
            miyu_home: Some(scratch.path().join("data").into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        };
        let root = DataRoot::locate(&env).unwrap();
        let packages = Packages::new(&resources, &root);
        Places { scratch, packages }
    }

    /// 在一层里写一份文件。
    fn write(&self, layer: Layer, file: &str, text: &str) {
        let base = match layer {
            Layer::Shipped => "res/packages",
            Layer::Home => "data/packages",
        };
        let path = self.scratch.path().join(base).join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

/// 一份最小的界面清单，子命令名是 `command`。
fn ui(command: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"{command}\" }}\n\n[command]\nname = \"{command}\"\nprogram = \"miyu-{command}\"\nabout = {{ en = \"Open {command}\" }}\n\n[ui]\n"
    )
}

/// 读到的：编号、层、读成了没有（问题的代码）。
fn brief(found: &[Found]) -> Vec<(String, Layer, Option<Code>)> {
    found
        .iter()
        .map(|found| {
            let code = match &found.read {
                Ok(_) => None,
                Err(Issue::Wrong(problem)) => Some(problem.code),
                Err(Issue::Unreadable(_)) => Some(Code::Syntax),
            };
            (found.id.clone(), found.layer, code)
        })
        .collect()
}

#[test]
fn both_layers_are_read_in_order_of_id() {
    let places = Places::new();
    places.write(Layer::Shipped, "web/package.toml", &ui("web"));
    places.write(Layer::Home, "tui/package.toml", &ui("tui"));
    places.write(Layer::Home, "notes.txt", "不是清单");
    places.write(Layer::Home, "empty/notes.toml", "文件夹里没有清单");
    places.write(Layer::Home, "old.toml", &ui("old"));
    places.write(Layer::Home, "Bad/package.toml", &ui("bad"));
    let found = places.packages.read();
    assert_eq!(
        brief(&found),
        [
            ("tui".to_string(), Layer::Home, None),
            ("web".to_string(), Layer::Shipped, None),
        ],
        "不是文件夹的、文件夹里没有 package.toml 的、编号不合写法的不算"
    );
    let web = found.iter().find(|found| found.id == "web").unwrap();
    assert_eq!(web.read.as_ref().unwrap().kind, PackageKind::Ui);
    assert!(web.path.ends_with("res/packages/web/package.toml"));
}

#[test]
fn nothing_installed_is_an_empty_list() {
    assert!(Places::new().packages.read().is_empty());
}

#[test]
fn the_shipped_one_wins_a_duplicate_id() {
    let places = Places::new();
    places.write(Layer::Shipped, "web/package.toml", &ui("web"));
    places.write(Layer::Home, "web/package.toml", &ui("web2"));
    assert_eq!(
        brief(&places.packages.read()),
        [
            ("web".to_string(), Layer::Shipped, None),
            ("web".to_string(), Layer::Home, Some(Code::Duplicate)),
        ]
    );
}

#[test]
fn a_command_name_is_taken_by_whoever_is_read_first() {
    let places = Places::new();
    places.write(Layer::Shipped, "web/package.toml", &ui("open"));
    places.write(Layer::Shipped, "zeta/package.toml", &ui("open"));
    places.write(Layer::Home, "alpha/package.toml", &ui("open"));
    let found = places.packages.read();
    assert_eq!(
        brief(&found),
        [
            ("alpha".to_string(), Layer::Home, Some(Code::CommandTaken)),
            ("web".to_string(), Layer::Shipped, None),
            ("zeta".to_string(), Layer::Shipped, Some(Code::CommandTaken)),
        ],
        "出厂的先于家目录，同一层照编号"
    );
    let Err(Issue::Wrong(problem)) = &found[0].read else {
        panic!("撞了");
    };
    assert_eq!(problem.detail, "open");
    assert_eq!(problem.line, Some(6), "报在子命令名那一行");
}

/// 加两个子命令的界面清单（施工 F-13）。
fn two(first: &str, second: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"{first}\" }}\n\n[[command]]\nname = \"{first}\"\nprogram = \"miyu-{first}\"\nabout = {{ en = \"Open\" }}\n\n[[command]]\nname = \"{second}\"\nprogram = \"miyu-{first}\"\nargs = [\"{second}\"]\nabout = {{ en = \"Open\" }}\n\n[ui]\n"
    )
}

#[test]
fn every_command_of_a_package_takes_its_name() {
    let places = Places::new();
    places.write(Layer::Shipped, "tui/package.toml", &two("tui", "shellhook"));
    places.write(Layer::Shipped, "web/package.toml", &two("web", "shellhook"));
    places.write(Layer::Shipped, "zeta/package.toml", &ui("web"));
    let found = places.packages.read();
    assert_eq!(
        brief(&found),
        [
            ("tui".to_string(), Layer::Shipped, None),
            ("web".to_string(), Layer::Shipped, Some(Code::CommandTaken)),
            ("zeta".to_string(), Layer::Shipped, None),
        ],
        "第二个子命令撞了，整份不收；它的 web 也不算占了"
    );
    let Err(Issue::Wrong(problem)) = &found[1].read else {
        panic!("撞了");
    };
    assert_eq!(problem.detail, "shellhook");
    assert_eq!(problem.line, Some(11), "报在被占的那个子命令名那一行");
}

/// 一份内置包的清单，带一个功能 `feature`，写在第 6 行。
fn builtin(feature: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"B\" }}\n\n[features.{feature}]\nname = {{ en = \"F\" }}\n\n[builtin]\n"
    )
}

/// 功能的编号（施工 F-1，设计 30 第三节）：两个包撞了，照读的先后（出厂的先于家目录，同一层照编号）先到先得，后到的那一份报
/// `feature_taken`、整份不收，报在那个功能那一行；没写功能的包，那一个功能的编号就是包的编号。
#[test]
fn a_feature_id_is_taken_by_whoever_is_read_first() {
    let places = Places::new();
    places.write(Layer::Shipped, "basesystem/package.toml", &builtin("files"));
    places.write(
        Layer::Shipped,
        "files/package.toml",
        "[package]\nprotocol = [1, 1]\nname = { en = \"Files\" }\n\n[command]\nname = \"files\"\nprogram = \"miyu-files\"\nabout = { en = \"F\" }\n\n[process]\n",
    );
    places.write(Layer::Home, "alpha/package.toml", &builtin("files"));
    places.write(Layer::Home, "beta/package.toml", &builtin("beta"));
    let found = places.packages.read();
    assert_eq!(
        brief(&found),
        [
            ("alpha".to_string(), Layer::Home, Some(Code::FeatureTaken)),
            ("basesystem".to_string(), Layer::Shipped, None),
            ("beta".to_string(), Layer::Home, None),
            (
                "files".to_string(),
                Layer::Shipped,
                Some(Code::FeatureTaken)
            ),
        ]
    );
    let Err(Issue::Wrong(problem)) = &found[0].read else {
        panic!("撞了");
    };
    assert_eq!((problem.detail.as_str(), problem.line), ("files", Some(5)));
    let Err(Issue::Wrong(problem)) = &found[3].read else {
        panic!("没写功能的照包的编号算");
    };
    assert_eq!((problem.detail.as_str(), problem.line), ("files", None));
}

/// 前缀（施工 PN-1，设计 48 第二节第 3 条）：照读的先后先到先得，后到的那一份报 `prefix_taken`、整份不收；没写的照包编号。
#[test]
fn a_prefix_is_taken_by_whoever_is_read_first() {
    let places = Places::new();
    let short = "[package]\nname = { en = \"T\" }\nprefix = \"tui\"\n";
    places.write(Layer::Shipped, "miyu-tui/package.toml", short);
    places.write(
        Layer::Home,
        "tui/package.toml",
        "[package]\nname = { en = \"T\" }\n",
    );
    places.write(
        Layer::Home,
        "web/package.toml",
        "[package]\nname = { en = \"W\" }\n",
    );
    let found = places.packages.read();
    assert_eq!(
        brief(&found),
        [
            ("miyu-tui".to_string(), Layer::Shipped, None),
            ("tui".to_string(), Layer::Home, Some(Code::PrefixTaken)),
            ("web".to_string(), Layer::Home, None),
        ],
        "出厂的先读到"
    );
    let Err(Issue::Wrong(problem)) = &found[1].read else {
        panic!("撞了");
    };
    assert_eq!(problem.detail, "tui");
}

#[test]
fn a_broken_manifest_is_listed_with_its_problem() {
    let places = Places::new();
    places.write(
        Layer::Shipped,
        "web/package.toml",
        "[package]\n\n[ui]\n\n[process]\n",
    );
    assert_eq!(
        brief(&places.packages.read()),
        [("web".to_string(), Layer::Shipped, Some(Code::TwoPrograms))]
    );
}

#[test]
fn the_state_dir_is_under_the_state_root() {
    let places = Places::new();
    assert!(
        places
            .packages
            .state_dir("tui")
            .ends_with("data/state/packages/tui")
    );
}

/// 每一种问题（加上协议版本对不上的那一句）三种语言都有给人看的一句，`{detail}` 换得进去（施工 9-1 上、下）。
#[test]
fn every_problem_code_is_said_in_three_languages() {
    let resources =
        ResourceRoot::at(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let mut codes: Vec<&str> = Code::ALL.iter().map(|code| code.as_str()).collect();
    let unique: std::collections::BTreeSet<&str> = codes.iter().copied().collect();
    assert_eq!(unique.len(), codes.len(), "写法不重复");
    codes.push("protocol_mismatch");
    // 跑包自己的检查时出的（施工 9-2，`miyu-endpoint` 的 `check/run.rs`）。
    codes.extend(["check_failed", "check_unavailable", "check_output"]);
    for language in ["zh", "en", "ja"] {
        let human = crate::human::Human::load(&resources, language).unwrap();
        for code in &codes {
            let said = miyu_config::Words::sentence(
                &human,
                &format!("package-problems/{code}"),
                &[("detail", "X")],
            );
            assert!(said.is_some(), "{language} 少了 {code}");
        }
    }
}

/// 包里的程序在哪（施工 F-8 上再补）：先找包文件夹的 `bin/`，再找主程序旁边的；包文件夹是相对路径的不找 `bin/`；都没有的
/// 是没有。只看在不在：空文件就够。
#[test]
fn a_program_is_found_in_the_package_bin_then_beside_main() {
    let places = Places::new();
    places.write(Layer::Home, "own/package.toml", &ui("own"));
    let file = |name: &str| format!("{name}{}", std::env::consts::EXE_SUFFIX);
    let beside = places.scratch.path().join("usr/bin");
    fs::create_dir_all(&beside).unwrap();
    let main = beside.join(file("miyu"));
    fs::write(&main, "").unwrap();
    let found = places.packages.read();
    let own = found.iter().find(|one| one.id == "own").unwrap();
    assert_eq!(own.program("miyu-own", &main), None, "哪儿都没有");

    fs::write(beside.join(file("miyu-own")), "").unwrap();
    let real = fs::canonicalize(&beside).unwrap();
    assert_eq!(
        own.program("miyu-own", &main),
        Some(Launch::direct(real.join(file("miyu-own")))),
        "只有主程序旁边有的照旁边的"
    );

    let bin = own.files_dir().join(BIN);
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join(file("miyu-own")), "").unwrap();
    assert_eq!(
        own.program("miyu-own", &main),
        Some(Launch::direct(bin.join(file("miyu-own")))),
        "包自己带的先"
    );
    // 相对的包文件夹不找 `bin/`：拼一个从当前目录走得到同一个 `bin/` 的相对路径，照样不认。
    #[cfg(unix)]
    {
        let cwd = std::env::current_dir().unwrap();
        let up: PathBuf = cwd.components().skip(1).map(|_| "..").collect();
        let relative = up.join(own.files_dir().strip_prefix("/").unwrap());
        assert!(relative.join(BIN).join(file("miyu-own")).is_file());
        assert_eq!(
            locate("miyu-own", &relative, &main),
            Some(Launch::direct(real.join(file("miyu-own")))),
            "相对的包文件夹不找 bin/"
        );
    }
}

/// Windows 上包里的程序可以是带 `#!` 的脚本（施工 X-34）：`.exe` 先；都没有的照 `bin/<名字>` 的 `#!` 起；没有 `#!`、`#!` 后面
/// 什么都没写的不算；Linux、macOS 上 `bin/<名字>` 本来就直接起。照 Windows 的找法在哪个平台上都走一遍。
#[test]
fn on_windows_a_script_in_bin_starts_by_its_shebang() {
    let places = Places::new();
    let dir = places.scratch.path().join("pkg");
    let bin = dir.join(BIN);
    fs::create_dir_all(&bin).unwrap();
    let main = places.scratch.path().join("usr/bin/miyu.exe");
    let script = bin.join("miyu-hello");
    fs::write(&script, "#!/usr/bin/env python3\nprint('hi')\n").unwrap();
    assert_eq!(
        locate_on("miyu-hello", &dir, &main, true),
        Some(Launch {
            program: PathBuf::from("python"),
            args: vec![script.clone().into_os_string()],
        }),
        "照 #! 找解释器"
    );
    assert_eq!(
        locate_on("miyu-hello", &dir, &main, false),
        Some(Launch::direct(script.clone())),
        "Linux、macOS 上直接起"
    );
    fs::write(bin.join("miyu-hello.exe"), "").unwrap();
    assert_eq!(
        locate_on("miyu-hello", &dir, &main, true),
        Some(Launch::direct(bin.join("miyu-hello.exe"))),
        ".exe 先"
    );
    fs::remove_file(bin.join("miyu-hello.exe")).unwrap();
    for first in ["print('hi')\n", "#!\n"] {
        fs::write(&script, first).unwrap();
        assert_eq!(
            locate_on("miyu-hello", &dir, &main, true),
            None,
            "第一行是 {first:?} 的不算"
        );
    }
    // 程序名带 `.py` 的也照 `#!`：没有的不照扩展名找解释器。
    fs::write(bin.join("miyu-hello.py"), "print('hi')\n").unwrap();
    assert_eq!(
        locate_on("miyu-hello.py", &dir, &main, true),
        None,
        "没有 #! 的不照扩展名起"
    );
}

/// 两个包带了同名的 MCP 服务（施工 X-2）：功能编号 `mcp-<服务名>` 撞了，照功能编号撞了报，后到的整份不收、报在那一段。
#[test]
fn an_mcp_server_name_is_taken_by_whoever_is_read_first() {
    let places = Places::new();
    let bundle = "[package]\nname = { en = \"G\" }\n\n[[mcp]]\nname = \"github\"\ncommand = [\"npx\", \"server-github\"]\n";
    places.write(Layer::Home, "alpha/package.toml", bundle);
    places.write(Layer::Home, "beta/package.toml", bundle);
    let found = places.packages.read();
    assert_eq!(
        brief(&found),
        [
            ("alpha".to_string(), Layer::Home, None),
            ("beta".to_string(), Layer::Home, Some(Code::FeatureTaken)),
        ]
    );
    let Err(Issue::Wrong(problem)) = &found[1].read else {
        panic!("撞了");
    };
    assert_eq!(
        (problem.detail.as_str(), problem.line),
        ("mcp-github", Some(4))
    );
}
