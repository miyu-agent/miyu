//! 三层谁盖谁（施工 SD-1）、卸掉的记号管到系统加装的，和照盖的先后先到先得的几样（子命令名、功能、前缀、MCP 服务名）。

use super::*;

/// 读到的：编号、层、版本、被哪一层盖住（没被盖住的没有）。
fn seen(found: &[Found]) -> Vec<(String, Layer, Option<String>, Option<Layer>)> {
    found
        .iter()
        .map(|found| {
            let (version, by) = match &found.read {
                Ok(manifest) => (manifest.version.clone(), None),
                Err(Issue::Covered { by, version }) => (version.clone(), Some(*by)),
                Err(_) => panic!("{} 读不成", found.id),
            };
            (found.id.clone(), found.layer, version, by)
        })
        .collect()
}

/// 一份带版本的界面清单。
fn versioned(command: &str, version: &str) -> String {
    ui(command).replacen(
        "[package]\n",
        &format!("[package]\nversion = \"{version}\"\n"),
        1,
    )
}

/// 同一个编号：装上的盖过系统加装的，系统加装的盖过出厂的，系统加装几处先列的优先；被盖住的记着盖它的那一层和自己的版本，
/// 不算错（施工 SD-1，原来的 `duplicate` 去掉了）。认的那一份排在最前，照编号找到的就是它。
#[test]
fn an_upper_layer_covers_the_same_id_below() {
    let places = Places::new();
    places.write(
        Layer::Shipped,
        "web/package.toml",
        &versioned("web", "1.0.0"),
    );
    places.write(
        Layer::Distro,
        "web/package.toml",
        &versioned("web", "1.1.0"),
    );
    places.write(Layer::Home, "web/package.toml", &versioned("web", "1.2.0"));
    places.write(
        Layer::Shipped,
        "tui/package.toml",
        &versioned("tui", "1.0.0"),
    );
    places.write(
        Layer::Distro,
        "tui/package.toml",
        &versioned("tui", "1.1.0"),
    );
    let second = places.scratch.path().join("distro2/tui/package.toml");
    fs::create_dir_all(second.parent().unwrap()).unwrap();
    fs::write(&second, versioned("tui", "1.5.0")).unwrap();
    let found = places.packages.read();
    let one = |text: &str| Some(text.to_string());
    assert_eq!(
        seen(&found),
        [
            ("tui".to_string(), Layer::Distro, one("1.1.0"), None),
            (
                "tui".to_string(),
                Layer::Distro,
                one("1.5.0"),
                Some(Layer::Distro)
            ),
            (
                "tui".to_string(),
                Layer::Shipped,
                one("1.0.0"),
                Some(Layer::Distro)
            ),
            ("web".to_string(), Layer::Home, one("1.2.0"), None),
            (
                "web".to_string(),
                Layer::Distro,
                one("1.1.0"),
                Some(Layer::Home)
            ),
            (
                "web".to_string(),
                Layer::Shipped,
                one("1.0.0"),
                Some(Layer::Home)
            ),
        ],
        "系统加装几处先列的优先，不管版本"
    );
    let tui = found.iter().find(|found| found.id == "tui").unwrap();
    assert!(
        tui.path.starts_with(places.scratch.path().join("distro")),
        "照编号找到的是认的那一份"
    );
    assert_eq!(places.packages.distro().len(), 2);
}

/// 卸掉的记号（`etc/removed/`）也管系统加装的：那一编号出厂的、系统加装的都不算装了；装回的时候读出盖得住的那一份。
#[test]
fn a_removed_mark_hides_the_distro_copy_too() {
    let places = Places::new();
    places.write(
        Layer::Shipped,
        "web/package.toml",
        &versioned("web", "1.0.0"),
    );
    places.write(
        Layer::Distro,
        "web/package.toml",
        &versioned("web", "1.1.0"),
    );
    places.write(
        Layer::Distro,
        "extra/package.toml",
        &versioned("extra", "0.1.0"),
    );
    install::mark_removed(&places.packages.removed_markers(), "web").unwrap();
    install::mark_removed(&places.packages.removed_markers(), "extra").unwrap();
    assert!(places.packages.read().is_empty());
    let removed = places.packages.read_removed();
    let shown: Vec<(&str, Layer)> = removed
        .iter()
        .map(|one| (one.id.as_str(), one.layer))
        .collect();
    assert_eq!(shown, [("extra", Layer::Distro), ("web", Layer::Distro)]);
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
            ("alpha".to_string(), Layer::Home, None),
            ("web".to_string(), Layer::Shipped, Some(Code::CommandTaken)),
            ("zeta".to_string(), Layer::Shipped, Some(Code::CommandTaken)),
        ],
        "照盖的先后：装上的先于出厂的，同一层照编号（施工 SD-1）"
    );
    let Err(Issue::Wrong(problem)) = &found[1].read else {
        panic!("撞了");
    };
    assert_eq!(problem.detail, "open");
    assert_eq!(problem.line, Some(6), "报在子命令名那一行");
}

/// 功能的编号（施工 F-1，设计 30 第三节）：两个包撞了，照读的先后（照盖的先后：装上的先于出厂的，同一层照编号，施工 SD-1）先到先得，后到的那一份报
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
            ("alpha".to_string(), Layer::Home, None),
            (
                "basesystem".to_string(),
                Layer::Shipped,
                Some(Code::FeatureTaken)
            ),
            ("beta".to_string(), Layer::Home, None),
            (
                "files".to_string(),
                Layer::Shipped,
                Some(Code::FeatureTaken)
            ),
        ]
    );
    let Err(Issue::Wrong(problem)) = &found[1].read else {
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
            (
                "miyu-tui".to_string(),
                Layer::Shipped,
                Some(Code::PrefixTaken)
            ),
            ("tui".to_string(), Layer::Home, None),
            ("web".to_string(), Layer::Home, None),
        ],
        "装上的先读到（施工 SD-1）"
    );
    let Err(Issue::Wrong(problem)) = &found[0].read else {
        panic!("撞了");
    };
    assert_eq!(problem.detail, "tui");
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
