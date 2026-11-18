//! 这台机器上装了的功能（施工 F-3 上，设计 `30-插件框架.md` 第三节）：预设照功能开关，工具挂在功能下。表由核心照读成了的
//! 软件包清单拼好交进来（`miyu-endpoint` 的 `presets::features`），这里只认编号：功能在哪个包、写明了哪几件工具。纯逻辑。

use std::collections::{BTreeMap, BTreeSet};

/// 一个装了的功能。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feature {
    /// 编号，全局不重。
    pub id: String,
    /// 它所在的软件包的编号。
    pub package: String,
    /// 清单里写明归它的工具；没写的是空的（包只有它一个功能的，工具都归它）。
    pub tools: Vec<String>,
    /// 是哪个 MCP 服务的（施工 X-2）：工具名以 `mcp__<服务名>__` 开头的归它；包自己的功能没有。
    pub service: Option<String>,
    /// 是哪个脚本的（施工 X-12 中，`skills.md`「脚本」）：叫这个名字的那一件工具归它；别的功能没有。
    pub script: Option<String>,
}

/// 装了的功能，照清单读的先后（包照编号，包里照写的先后）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Features {
    list: Vec<Feature>,
    /// 清单写了 `needs = ["images"]` 的包（施工 X-25）：显示不了图片的会话里整包当没开（[`crate::preset::Chosen::situated`]）。
    images: BTreeSet<String>,
    /// 接平台的包（施工 IM-4）：包的编号到清单 `[connection] platform` 写的平台。在自己平台的场所会话里整包不过预设开关
    /// （[`crate::preset::Chosen::situated`]）。
    connections: BTreeMap<String, BTreeSet<String>>,
    /// 扩展这时替这个人交的功能（施工 IM-6，`feature.offer`）：归交的那个包；列出来的工具归它，不归包自己的功能。
    offered: Vec<Feature>,
}

impl Features {
    /// 照 `list` 的先后。
    pub fn new(list: Vec<Feature>) -> Features {
        Features {
            list,
            images: BTreeSet::new(),
            connections: BTreeMap::new(),
            offered: Vec::new(),
        }
    }

    /// 记下哪几个包要这个地方显示得了图片（施工 X-25，清单的 `[package] needs`）。
    #[must_use]
    pub fn needing_images(mut self, packages: impl IntoIterator<Item = String>) -> Features {
        self.images = packages.into_iter().collect();
        self
    }

    /// 记下接平台的包和它们接的平台（施工 IM-4，清单的 `[connection] platform`）。
    #[must_use]
    pub fn connecting(mut self, connections: BTreeMap<String, BTreeSet<String>>) -> Features {
        self.connections = connections;
        self
    }

    /// 换上扩展交的功能（施工 IM-6）：整份替换。
    #[must_use]
    pub fn offering(mut self, offered: Vec<Feature>) -> Features {
        self.offered = offered;
        self
    }

    /// 扩展交的功能（施工 IM-6）。
    pub fn offered(&self) -> &[Feature] {
        &self.offered
    }

    /// 接平台的包和它们接的平台（施工 IM-4）。
    pub fn connections(&self) -> &BTreeMap<String, BTreeSet<String>> {
        &self.connections
    }

    /// 场所 `venue` 的平台（第一个 `:` 前面那一段）是哪几个包接的（施工 IM-4）：本机的、没有平台前缀的一个都没有。
    pub fn connected_to(&self, venue: &str) -> BTreeSet<String> {
        let Some((platform, _)) = venue.split_once(':') else {
            return BTreeSet::new();
        };
        self.connections
            .iter()
            .filter(|(_, platforms)| platforms.contains(platform))
            .map(|(package, _)| package.clone())
            .collect()
    }

    /// 要这个地方显示得了图片的包（施工 X-25），照编号排。
    pub fn image_packages(&self) -> &BTreeSet<String> {
        &self.images
    }

    /// 每一个，照先后；扩展交的（施工 IM-6）接在最后。
    pub fn iter(&self) -> impl Iterator<Item = &Feature> {
        self.list.iter().chain(&self.offered)
    }

    /// 功能 `id` 装了没有。
    pub fn installed(&self, id: &str) -> bool {
        self.iter().any(|feature| feature.id == id)
    }

    /// 包 `package` 里的工具 `tool` 归哪个功能：MCP 服务的工具（`mcp__<服务名>__…`）归那个服务的功能（施工 X-2），脚本的工具
    /// 归那个脚本的功能（施工 X-12 中）；别的照包自己的功能算（MCP 服务的、脚本的不算在里面）：清单里写明了的照写的，包只有
    /// 一个功能的归它；都不是的（没装的包、写了几个功能又没列它的）没有。扩展交的功能列了的先归它（施工 IM-6），不算进「包只有
    /// 一个功能」。
    pub fn of_tool(&self, package: &str, tool: &str) -> Option<&str> {
        // 扩展交的功能列了的先认（施工 IM-6）：它只列自己登记的工具。
        if let Some(offered) = self.offered.iter().find(|feature| {
            feature.package == package && feature.tools.iter().any(|name| name == tool)
        }) {
            return Some(&offered.id);
        }
        let in_package = self
            .list
            .iter()
            .filter(|feature| feature.package == package);
        if let Some(service) = in_package.clone().find(|feature| {
            feature.service.as_deref().is_some_and(|service| {
                tool.strip_prefix("mcp__")
                    .and_then(|rest| rest.strip_prefix(service))
                    .is_some_and(|rest| rest.starts_with("__"))
            })
        }) {
            return Some(&service.id);
        }
        if let Some(script) = in_package
            .clone()
            .find(|feature| feature.script.as_deref() == Some(tool))
        {
            return Some(&script.id);
        }
        let mut own =
            in_package.filter(|feature| feature.service.is_none() && feature.script.is_none());
        if let Some(listed) = own
            .clone()
            .find(|feature| feature.tools.iter().any(|name| name == tool))
        {
            return Some(&listed.id);
        }
        match (own.next(), own.next()) {
            (Some(only), None) => Some(&only.id),
            _ => None,
        }
    }

    /// 以前的快照记的没开的软件 `ids`（施工 P-2 中起记的是包的编号）照现在的功能读：是功能编号的照旧，是装了的包、又不是
    /// 功能编号的换成这个包的全部功能，别的照旧（施工 F-3 上：比较预设改没改时用，升级以后不白白换一次快照）。
    pub fn read_legacy<'a>(&self, ids: impl IntoIterator<Item = &'a String>) -> BTreeSet<String> {
        let mut read = BTreeSet::new();
        for id in ids {
            let expanded: Vec<&Feature> = match self.installed(id) {
                true => Vec::new(),
                false => self
                    .list
                    .iter()
                    .filter(|feature| feature.package == *id)
                    .collect(),
            };
            match expanded.is_empty() {
                true => {
                    read.insert(id.clone());
                }
                false => read.extend(expanded.iter().map(|feature| feature.id.clone())),
            }
        }
        read
    }
}

#[cfg(test)]
mod tests;
