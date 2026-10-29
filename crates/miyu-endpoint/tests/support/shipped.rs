//! 出厂那一层的夹具扩展（施工 S-6 下）：接入QQ 搬出了核心仓库，出厂的包里不再有扩展（带命令、要的能力、配置项的）。
//! 要测「出厂的扩展」怎样的，把源码树的资源目录整个拷一份，`packages/` 下多放这份测试自己写的清单 [`COURIER`]，核心照这份
//! 资源目录造。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use miyu_endpoint::Core;
use miyu_session::testkit::Script;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use super::{Home, TOKEN, default_resources};

/// 夹具扩展的编号：子命令名也是它。
pub const ID: &str = "courier";

/// 夹具扩展的清单：带子命令、手动开、要六样能力、一项只能写配置的配置项 `courier.listen`。程序一定不在。
pub const COURIER: &str = "[package]\nprotocol = [1, 1]\nname = { en = \"Courier\", zh = \"信使\" }\n\n[command]\nname = \"courier\"\nprogram = \"miyu-no-such-program-anywhere\"\nabout = { en = \"Courier\" }\n\n[process]\nargs = [\"serve\"]\nstart = \"manual\"\ncapabilities = [\"tools\", \"sessions.drive\", \"act_for_external\", \"events.read\", \"events.write\", \"network\"]\n\n[settings.listen]\ntype = \"int\"\nmin = 1024\nmax = 65535\ndefault = 8301\nlayers = [\"system\"]\napplies = \"now\"\nname = { en = \"Port\" }\n";

/// 拷一份源码树的资源目录到 `home.work` 下，出厂那一层多一个 [`COURIER`]，交回这份资源目录。
pub fn resources(home: &Home) -> PathBuf {
    let resources = home.work.join("resources");
    copy_tree(&default_resources(), &resources);
    let package = resources.join("packages").join(ID);
    std::fs::create_dir_all(&package).expect("建得了");
    std::fs::write(package.join("package.toml"), COURIER).expect("写得进");
    resources
}

/// 一份核心，资源目录是带着夹具扩展的那一份（[`resources`]）；要别的设定的照 `Core` 的 `with_…` 接着设。
pub fn core(home: &Home, resources: &Path, script: &Script) -> Core {
    Core::new(
        home.root.clone(),
        ResourceRoot::at(resources),
        Arc::new(script.clone()),
        Catalog::default(),
        None,
        TOKEN.to_string(),
    )
}

/// 把 `from` 整棵拷到 `to`。
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("建得了");
    for entry in std::fs::read_dir(from).expect("读得了") {
        let entry = entry.expect("读得了");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("认得出种类").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("拷得了");
        }
    }
}
