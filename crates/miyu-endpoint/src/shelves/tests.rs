//! 每人一个工具目录架子（施工 U23 中中）：机主、包的账号（平台桥）拿核心那一个，看得到机主私人的 `~home`；成员拿叠在上面的，
//! 看不到；核心那一个换了，成员的跟着换。

use std::sync::Arc;

use miyu_kernel::id::AccountId;
use miyu_kernel::tool::Access;
use miyu_session::testkit::Script;
use miyu_store::resources::ResourceRoot;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::Core;
use crate::test_support::{resources, temp_root};

/// 一件只读的假工具。
fn tool(name: &str) -> Arc<dyn Tool> {
    Fake::new(name, Access::Read, Act::Echo)
}

fn names(catalog: &Catalog) -> Vec<String> {
    catalog.specs().map(|spec| spec.name.clone()).collect()
}

#[test]
fn the_owner_s_side_shares_the_core_shelf_and_others_get_one_without_home() {
    let (root, dir) = temp_root("shelves");
    let courier = root.path().join("system/packages/courier");
    std::fs::create_dir_all(&courier).expect("建得了");
    std::fs::write(
        courier.join("package.toml"),
        "[package]\nprotocol = [1, 1]\nname = { en = \"Courier\" }\n\n[command]\nname = \"courier\"\nprogram = \"miyu-no-such-program-anywhere\"\nabout = { en = \"C\" }\n\n[process]\nsystem_account = true\n",
    )
    .expect("写得进");
    let catalog = Catalog::in_packages([
        ("basesystem", vec![tool("read")]),
        ("~home", vec![tool("weather")]),
    ])
    .expect("合写法");
    let core = Arc::new(Core::new(
        root,
        ResourceRoot::at(resources()),
        Arc::new(Script::new([])),
        catalog,
        None,
        AccountId::parse("alice").expect("合写法"),
        "token".to_string(),
    ));
    let shelf = |name: &str| core.shelf_of(&AccountId::parse(name).expect("合写法"));
    assert_eq!(
        names(&shelf("alice").current()),
        ["read", "weather"],
        "机主的照旧"
    );
    assert_eq!(
        names(&shelf("courier").current()),
        ["read", "weather"],
        "平台桥（场所会话）照旧"
    );
    assert_eq!(names(&shelf("bob").current()), ["read"], "成员的拿掉 ~home");
    assert!(shelf("bob").get("weather").is_none(), "执行时也找不到");
    let before = shelf("bob").edition().generation;
    core.tools
        .replace(|catalog| catalog.replacing("kit", vec![tool("dice")]))
        .expect("换得了");
    assert_eq!(
        names(&shelf("bob").current()),
        ["dice", "read"],
        "核心那一个换了跟着换"
    );
    assert!(shelf("bob").edition().generation > before, "代数往上走");
    assert_eq!(
        names(&shelf("alice").current()),
        ["dice", "read", "weather"]
    );
    drop(core);
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        eprintln!("临时目录没删掉：{error}");
    }
}

/// 核心不让成员的架子多活（施工 U23 中下补）：拿着它的（他的会话）都丢了，架子连同挂在上面看着他 `scripts/` 的监视一起放掉；再要
/// 的时候现造一个，照样拿掉 `~home`。还拿着的时候要的是同一个。
#[test]
fn a_member_s_shelf_and_its_watch_go_when_no_session_holds_it() {
    let (root, dir) = temp_root("shelves-weak");
    let catalog = Catalog::in_packages([
        ("basesystem", vec![tool("read")]),
        ("~home", vec![tool("weather")]),
    ])
    .expect("合写法");
    let core = Arc::new(Core::new(
        root,
        ResourceRoot::at(resources()),
        Arc::new(Script::new([])),
        catalog,
        None,
        AccountId::parse("alice").expect("合写法"),
        "token".to_string(),
    ));
    let bob = AccountId::parse("bob").expect("合写法");
    let held = core.shelf_of(&bob);
    let weak = held.downgrade();
    assert!(
        core.shelf_of(&bob).downgrade().upgrade().is_some() && weak.upgrade().is_some(),
        "还拿着的时候照旧那一个"
    );
    drop(held);
    assert!(
        weak.upgrade().is_none(),
        "没人拿着了，核心不留着它（监视跟着放掉）"
    );
    assert_eq!(
        names(&core.shelf_of(&bob).current()),
        ["read"],
        "再要现造一个"
    );
    drop(core);
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        eprintln!("临时目录没删掉：{error}");
    }
}
