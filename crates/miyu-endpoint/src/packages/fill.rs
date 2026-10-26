//! `package.list` 一项里照清单写的几格（施工 9-1 上起，F-12 从 `packages.rs` 挪出来）：种类、名字、命令、功能、依赖、界面块……；
//! 作者、上游（施工 F-12）；这个包是从哪来的（[`from`]）。

use serde_json::{Value, json};

use miyu_config::package::{Manifest, PackageKind};
use miyu_store::packages::{Found, Layer, Packages};

use crate::personas::pick;

/// 读成了的几格；没有的不写。
pub(crate) fn fill(item: &mut Value, id: &str, manifest: &Manifest, language: &str) {
    item["kind"] = json!(manifest.kind.as_str());
    if let Some(protocol) = manifest.protocol {
        item["protocol"] = json!(protocol);
    }
    item["name"] = json!(pick(&manifest.name, language));
    // 归哪几个组（施工 BS-1 下）：头照它把组里的包收在一起。
    if !manifest.groups.is_empty() {
        item["groups"] = json!(manifest.groups);
    }
    if let Some(version) = &manifest.version {
        item["version"] = json!(version);
    }
    if let Some(summary) = pick(&manifest.summary, language) {
        item["summary"] = json!(summary);
    }
    // 第一个子命令照旧放在 `command`；一个包加几个的（施工 F-13）另交 `commands`，照写的先后。
    let commands: Vec<Value> = manifest
        .commands
        .iter()
        .map(|command| {
            json!({
                "name": command.name,
                "program": command.program,
                "about": pick(&command.about, language),
            })
        })
        .collect();
    if let Some(first) = commands.first() {
        item["command"] = first.clone();
        item["commands"] = json!(commands);
    }
    if let Some(process) = &manifest.process {
        item["process"] = json!({"args": process.args, "start": process.start.as_str()});
    }
    if let Some(ui) = &manifest.ui {
        item["opens"] = json!(ui.opens);
        if let Some(dir) = &ui.pages_dir {
            item["pages_dir"] = json!(dir);
        }
    }
    if let Some(check) = &manifest.check {
        item["check"] = json!({"args": check.args});
    }
    for (key, value) in [
        ("author", &manifest.author),
        ("upstream", &manifest.upstream),
    ] {
        if let Some(value) = value {
            item[key] = json!(value);
        }
    }
    // 给哪几个包带了内容（`[content.<包>]`，施工 G-9）：im 的商城照它挑 im 的插件；没带的不写，同别的格。
    if !manifest.content.is_empty() {
        item["contents"] = json!(
            manifest
                .content
                .iter()
                .map(|content| content.target.as_str())
                .collect::<Vec<_>>()
        );
    }
    links(item, id, manifest, language);
}

/// 施工 F-1 加的几格（设计 30）：带的功能（照包算的那一个也列，名字、说明照语言挑；不列工具）、平台接入、依赖、小程序。
fn links(item: &mut Value, id: &str, manifest: &Manifest, language: &str) {
    if matches!(
        manifest.kind,
        PackageKind::Builtin | PackageKind::Process | PackageKind::Bundle
    ) {
        let features: Vec<Value> = manifest
            .features_of(id)
            .iter()
            .map(|feature| {
                let mut listed = json!({"id": feature.id, "name": pick(&feature.name, language)});
                if let Some(summary) = pick(&feature.summary, language) {
                    listed["summary"] = json!(summary);
                }
                listed
            })
            .collect();
        item["features"] = json!(features);
    }
    if let Some(connection) = &manifest.connection {
        item["connection"] = json!({"platform": connection.platform});
    }
    // 依赖、推荐的包（施工 F-11：以前只写小程序，叫 `workers`）。
    for (key, packages) in [
        ("depends", &manifest.depends),
        ("recommends", &manifest.recommends),
    ] {
        if !packages.is_empty() {
            item[key] = json!({"packages": packages});
        }
    }
    // 给别的软件调的方法（施工 F-11，接入QQ 照它找挂在自己身上的包）。
    if !manifest.export.is_empty() {
        item["export"] = json!({"methods": manifest.export});
    }
    if let Some(worker) = &manifest.worker {
        item["worker"] = json!({"program": worker.program, "args": worker.args});
    }
    if let Some(icon) = &manifest.icon {
        item["icon"] = json!(icon);
    }
    if let Some(theme) = &manifest.theme {
        item["theme"] = json!({"file": theme.file});
    }
    if let Some(mascot) = &manifest.mascot {
        item["mascot"] = json!({"model": mascot.model});
    }
    // 带的 MCP 服务（施工 X-2）：名字、拉起它的命令（人看得到会在这台机器上跑什么）或者它的网址（施工 X-32 上）、给哪种会话；环境变量、
    // 令牌、请求头不列（可能引用密钥）。
    if !manifest.mcp.is_empty() {
        let servers: Vec<Value> = manifest
            .mcp
            .iter()
            .map(|server| {
                let mut one = json!({"name": server.name, "venues": server.venues});
                crate::mcp::methods::reach(server, &mut one);
                one
            })
            .collect();
        item["mcp"] = json!(servers);
    }
    // 界面块（施工 X-15 上，`widgets.md`）：头照它知道这个包在哪几个位置放东西、哪几页能开。
    if !manifest.widgets.is_empty() {
        let widgets: Vec<Value> = manifest
            .widgets
            .iter()
            .map(|widget| {
                let scope = if widget.per_session {
                    "session"
                } else {
                    "global"
                };
                let mut one =
                    json!({"id": widget.id, "place": widget.place.as_str(), "scope": scope});
                if let Some(title) = widget
                    .title
                    .as_ref()
                    .and_then(|title| pick(title, language))
                {
                    one["title"] = json!(title);
                }
                if let Some(command) = &widget.command {
                    one["command"] = json!(command);
                }
                one
            })
            .collect();
        item["widgets"] = json!(widgets);
    }
}

/// 包是从哪来的（施工 F-12）：出厂的、从官方软件源装的（装的时候记下了来源仓库）、本机文件夹或包文件装的。以后有了社区软件源
/// 再分出 `community`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum From {
    Shipped,
    Official,
    Local,
}

impl From {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            From::Shipped => "shipped",
            From::Official => "official",
            From::Local => "local",
        }
    }
}

/// `found` 是从哪来的：出厂那一层的是出厂；装进来的照装的时候记的来源仓库（`desc` 的 `origin`）认。碰磁盘：读一份小文件。
pub(crate) fn from(found: &Found, places: &Packages) -> From {
    if found.layer == Layer::Shipped {
        return From::Shipped;
    }
    match miyu_store::packages::local::origin(&places.local_root(), &found.id) {
        Some(_) => From::Official,
        None => From::Local,
    }
}
