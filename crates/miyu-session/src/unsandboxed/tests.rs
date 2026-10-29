//! 主人放行出沙盒的工具（施工 X-36）：照名字和来处认，来处对不上的、只写了名字没写对键的不算。带文件的（施工 X-37 补）：文件
//! 的真实位置落在管得住的地方的才算，链接指出去的、换不成真实位置的、没给管得住的地方的不算，链接指到里面的算。

use std::sync::Arc;

use miyu_kernel::tool::Access;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Shelf, Tool};

use super::*;

fn values(pairs: &[(&str, &str)]) -> Values {
    let mut values = Values::defaults(&[]);
    for (key, set) in pairs {
        values.set(key, Value::Text(set.to_string().into()));
    }
    values
}

/// 架子上：`~home` 的 `light`，`basesystem` 的 `shell`、`smart-home` 的 `lamp`。
fn shelf() -> Shelf {
    let fake = |name: &str| Fake::new(name, Access::Execute, Act::Echo) as Arc<dyn Tool>;
    Shelf::new(
        Catalog::in_packages([
            ("~home", vec![fake("light")]),
            ("basesystem", vec![fake("shell")]),
            ("smart-home", vec![fake("lamp")]),
        ])
        .expect("合写法"),
    )
}

#[test]
fn a_grant_counts_only_when_the_source_matches() {
    let table = Unsandboxed::of(&values(&[
        ("sandbox.tools.light", "~home"),
        ("sandbox.tools.shell", "basesystem"),
        ("sandbox.tools.lamp", "other-home"),
        ("sandbox.tools.gone", "~home"),
    ]));
    let shelf = shelf();
    assert_eq!(table.grant("light", &shelf), Grant::Whole);
    assert_eq!(table.grant("shell", &shelf), Grant::Whole, "shell 也认");
    assert_eq!(table.grant("lamp", &shelf), Grant::No, "来处对不上的不算");
    assert_eq!(table.grant("gone", &shelf), Grant::No, "目录里没有的不算");
    assert_eq!(table.grant("unlisted", &shelf), Grant::No, "没写的不算");
    assert_eq!(
        table.entries().collect::<Vec<_>>(),
        [
            ("gone", "~home"),
            ("lamp", "other-home"),
            ("light", "~home"),
            ("shell", "basesystem"),
        ]
    );
}

/// 临时目录里的一处，用完删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("miyu-unsandboxed-{name}-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("own")).unwrap();
        std::fs::create_dir_all(dir.join("elsewhere")).unwrap();
        std::fs::write(dir.join("own/run.py"), "x").unwrap();
        std::fs::write(dir.join("elsewhere/run.py"), "x").unwrap();
        Scratch(dir)
    }

    fn real(&self, path: &str) -> PathBuf {
        std::fs::canonicalize(self.0.join(path)).unwrap()
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 带文件的放行（施工 X-37 补）：几件都放行了、来处都对，只看文件这一刻的真实位置。
#[test]
fn a_tool_with_a_file_counts_only_where_the_admin_controls_it() {
    let site = Scratch::new("file");
    let file = |name: &str, path: PathBuf| {
        Fake::with_file(name, Access::Execute, path, Act::Echo) as Arc<dyn Tool>
    };
    // 链接的两件（指出去的、指到里面的）只在 Unix 上建：Windows 上建链接要权限。
    #[cfg(unix)]
    let linked = {
        std::os::unix::fs::symlink(site.0.join("elsewhere/run.py"), site.0.join("own/out.py"))
            .unwrap();
        std::os::unix::fs::symlink(site.0.join("own/run.py"), site.0.join("own/in.py")).unwrap();
        vec![
            file("linked-out", site.0.join("own/out.py")),
            file("linked-in", site.0.join("own/in.py")),
        ]
    };
    #[cfg(not(unix))]
    let linked = Vec::new();
    let tools = [
        vec![
            file("inside", site.0.join("own/run.py")),
            file("outside", site.0.join("elsewhere/run.py")),
            file("missing", site.0.join("own/gone.py")),
        ],
        linked,
    ]
    .concat();
    let names: Vec<String> = tools.iter().map(|tool| tool.spec().name.clone()).collect();
    let shelf = Shelf::new(Catalog::in_packages([("~home", tools)]).expect("合写法"));
    let pairs: Vec<(String, &str)> = names
        .iter()
        .map(|name| (format!("sandbox.tools.{name}"), "~home"))
        .collect();
    let pairs: Vec<(&str, &str)> = pairs
        .iter()
        .map(|(key, source)| (key.as_str(), *source))
        .collect();
    let table = Unsandboxed::of(&values(&pairs)).within(Confines::new(vec![site.real("own")]));
    assert_eq!(
        table.grant("inside", &shelf),
        Grant::File(site.real("own/run.py"))
    );
    assert_eq!(
        table.grant("outside", &shelf),
        Grant::No,
        "落在管得住的地方以外"
    );
    assert_eq!(
        table.grant("missing", &shelf),
        Grant::No,
        "换不成真实位置的"
    );
    #[cfg(unix)]
    {
        assert_eq!(table.grant("linked-out", &shelf), Grant::No, "链接指出去的");
        assert_eq!(
            table.grant("linked-in", &shelf),
            Grant::File(site.real("own/run.py")),
            "链接指到里面的照真实位置算"
        );
    }
    let bare = Unsandboxed::of(&values(&pairs));
    assert_eq!(
        bare.grant("inside", &shelf),
        Grant::No,
        "没给管得住的地方的一律不算"
    );
}

#[test]
fn only_sandbox_tools_keys_count() {
    let table = Unsandboxed::of(&values(&[
        ("sandbox.light", "~home"),
        ("clicall.tools.light", "~home"),
    ]));
    assert_eq!(table, Unsandboxed::default());
}

/// 工具名那一段带引号写的也认得（同 `clicall.tools.<name>`）。
#[test]
fn a_quoted_tool_name_is_read() {
    let table = Unsandboxed::of(&values(&[("sandbox.tools.\"mcp__home__Lamp\"", "~home")]));
    assert_eq!(
        table.entries().collect::<Vec<_>>(),
        [("mcp__home__Lamp", "~home")]
    );
}

/// 只认系统配置：写在项目配置里的读的时候就报出来，不进这一轮的值。
#[test]
fn a_project_config_cannot_grant() {
    let source = "[sandbox.tools]\nlight = \"~home\"\n";
    let project = miyu_config::parse::parse(ITEMS, Layer::Project, source).expect("写法对");
    assert!(!project.problems.is_empty(), "项目配置里不收");
    let system = miyu_config::parse::parse(ITEMS, Layer::System, source).expect("写法对");
    assert!(system.problems.is_empty(), "{:?}", system.problems);
}

/// 主人管得住的地方（施工 U23 上上、中下；SU-4 下起照会话关不关）：装上的包（`packages/`）、数据根里自己放的文件只对主人本人
/// 的会话算；关着的会话照沙盒跑；出厂的包谁都算。
#[test]
fn installed_packages_are_controlled_only_for_the_owner() {
    let site = Scratch::new("system");
    let env = miyu_store::env::Env {
        platform: miyu_store::env::Platform::current(),
        miyu_home: Some(site.0.join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    let root = DataRoot::locate(&env).expect("MIYU_HOME 是绝对路径");
    let path = site.0.join("data/packages/kit/scripts/run.py");
    std::fs::create_dir_all(path.parent().expect("有上级")).unwrap();
    std::fs::write(&path, "x").unwrap();
    let system = site.0.join("data/packages/kit/scripts/run.py");
    let owner = Confines::of(&root, true, &site.0.join("own"));
    assert_eq!(
        owner.hold(&system),
        Some(site.real("data/packages/kit/scripts/run.py")),
        "机主的算"
    );
    let member = Confines::of(&root, false, &site.0.join("own"));
    assert_eq!(member.hold(&system), None, "关着的会话不算，照沙盒跑");
    assert!(
        member.hold(&site.0.join("own/run.py")).is_some(),
        "出厂的包照旧算"
    );
}

/// 放行出沙盒只对主人本人的会话算（施工 U23 中下；SU-4 下起照会话关不关）：系统配置放行了 `weather`、来处是 `~home`，主人
/// 的会话照放行；关着的会话里同一个脚本照它的沙盒跑。
#[test]
fn a_confined_session_s_script_is_never_let_out_of_the_sandbox() {
    let site = Scratch::new("member-own");
    let env = miyu_store::env::Env {
        platform: miyu_store::env::Platform::current(),
        miyu_home: Some(site.0.join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    let root = DataRoot::locate(&env).expect("MIYU_HOME 是绝对路径");
    let script = || {
        let path = site.0.join("data/scripts/weather.py");
        std::fs::create_dir_all(path.parent().expect("有上级")).unwrap();
        std::fs::write(&path, "x").unwrap();
        let tool = Fake::with_file("weather", Access::Execute, path, Act::Echo) as Arc<dyn Tool>;
        Shelf::new(Catalog::in_packages([("~home", vec![tool])]).expect("合写法"))
    };
    let granted = || Unsandboxed::of(&values(&[("sandbox.tools.weather", "~home")]));
    // 文件先放好、数据根先有了再定管得住的地方：同开会话时（数据根早就在），`Confines::of` 照数据根的真实位置接（macOS 的临时
    // 目录在 `/var` 这条链接下面，Windows 的有短名），数据根还没有的取不到真实位置。
    let shelf = script();
    let owner = granted().within(Confines::of(&root, true, &site.0.join("own")));
    assert_eq!(
        owner.grant("weather", &shelf),
        Grant::File(site.real("data/scripts/weather.py")),
        "主人的会话照旧放行"
    );
    let member = granted().within(Confines::of(&root, false, &site.0.join("own")));
    assert_eq!(
        member.grant("weather", &shelf),
        Grant::No,
        "关着的会话不放行，照它的沙盒跑"
    );
}
