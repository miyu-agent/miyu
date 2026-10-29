//! 工具目录（05 第八节，施工 4-1）：核心起来时登记一次；提供者登记一次换出新的一份（施工 O-2 上，放在 [`crate::Shelf`]
//! 上）。照名字排好，造会话时照这个先后交出工具面。改过名的工具照以前的名字也找得到（施工 7-5 再补，[`Tool::formerly`]）。

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use serde_json::Value;

use crate::{Spec, Tool};

/// 工具名最长多少个字符：各家供应商对函数名的限制（05 第六节）。
const NAME_LIMIT: usize = 64;

/// 工具目录。
#[derive(Clone, Default)]
pub struct Catalog {
    tools: BTreeMap<String, Arc<dyn Tool>>,
    /// 以前的名字到现在的名字（施工 7-5 再补）。
    formerly: BTreeMap<String, String>,
    /// 每件工具现在的名字到它的软件包（施工 P-2 中）：预设照包开关。
    packages: BTreeMap<String, String>,
    /// 经提供者登记过的包（施工 O-2 中，[`Catalog::replacing`]）：回合开头换快照时，只有它们的工具照现在的登记换。
    provided: BTreeSet<String>,
    /// 随包卸掉的工具（施工 F-5 中，[`Catalog::removing`]）：工具名到它原来的包。用过它的会话调到时报「已卸载」，不是
    /// 「暂时不可用」。包装回来就不算了。
    gone: BTreeMap<String, String>,
    /// 脚本的工具（施工 X-12 中，[`Catalog::replacing_scripts`]）：下一次换的时候整份拿掉。
    scripts: BTreeSet<String>,
}

/// 基础系统的编号（`10-自带软件.md` 第三节）：只交一串工具的老写法，全算它。
pub const BASESYSTEM: &str = "basesystem";

/// MCP 服务的工具名的开头（施工 X-3 上，`docs/blueprint/mcp.md`「对外的样子」第 1 条）：`mcp__<服务名>__<工具名>`。包自己
/// 登记的工具不许用它（`provide` 拦下）。
pub const MCP_PREFIX: &str = "mcp__";

/// 服务 `service` 的工具名的开头：`mcp__<服务名>__`。
pub fn mcp_prefix(service: &str) -> String {
    format!("{MCP_PREFIX}{service}__")
}

/// MCP 工具名 `name` 是哪个服务的（施工 X-39）：`mcp__<服务名>__<工具名>` 的服务名；不是这个样子的没有。服务名里没有 `_`
/// （只有小写字母、数字、`-`），照第一个 `__` 切得开。
pub fn mcp_service(name: &str) -> Option<&str> {
    let (service, _) = name.strip_prefix(MCP_PREFIX)?.split_once("__")?;
    (!service.is_empty()).then_some(service)
}

/// 一件工具登记不上：是哪一件，哪一条没过。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogError {
    /// 那件工具的名字。
    pub tool: String,
    /// 哪一条没过。
    pub problem: Problem,
}

/// 登记时查的几条（05 第六节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// 已经有一件同名的：她调的是哪一件，说不清。以前的名字也算（施工 7-5 再补）：撞上别的工具现在的、以前的名字，
    /// 同样说不清。
    Duplicate,
    /// 名字不合写法：只用英文字母、数字、`_`、`-`，1 到 64 个字符。不合的，每次请求都会被供应商拒收。
    Name,
    /// 参数格式不是 `{"type":"object",…}`：供应商只收对象。
    Parameters,
}

impl Catalog {
    /// 登记这几件，照名字排好。有一件查不过，整个目录都登记不上，报交进来时排在前面的那一件。以前的名字跟着它现在的名字
    /// 登记，只查重名：撞上的报那个以前的名字。
    ///
    /// # Errors
    ///
    /// 有两件同名的（以前的名字也算）、名字不合写法的、参数格式不是对象的。
    pub fn new(tools: impl IntoIterator<Item = Arc<dyn Tool>>) -> Result<Catalog, CatalogError> {
        Catalog::in_packages([(BASESYSTEM, tools.into_iter().collect::<Vec<_>>())])
    }

    /// 照软件包登记（施工 P-2 中）：每一组是一个包的编号和它的几件，记下每件归哪个包；查法同 [`Catalog::new`]，两个包里
    /// 同名的也算重名。
    ///
    /// # Errors
    ///
    /// 同 [`Catalog::new`]。
    pub fn in_packages<'a>(
        groups: impl IntoIterator<Item = (&'a str, Vec<Arc<dyn Tool>>)>,
    ) -> Result<Catalog, CatalogError> {
        let mut catalog = Catalog::default();
        for (package, tools) in groups {
            for tool in tools {
                catalog.add(package, tool)?;
            }
        }
        Ok(catalog)
    }

    /// 换掉包 `package` 的工具（施工 O-2 上，提供者再登记一次）：交回新的一份，别的包的照留，原来这份不动；没有这个包的是
    /// 加进来。查法同 [`Catalog::new`]，撞上别的包的也算重名。它的 MCP 服务的工具不动（施工 X-3 上，
    /// [`Catalog::replacing_service`]）。
    ///
    /// # Errors
    ///
    /// 同 [`Catalog::new`]。
    pub fn replacing(
        &self,
        package: &str,
        tools: Vec<Arc<dyn Tool>>,
    ) -> Result<Catalog, CatalogError> {
        let mut catalog = self.clone();
        catalog.take_out_where(package, |name| !name.starts_with(MCP_PREFIX));
        // 卸掉以后装回来、又登记了的（施工 F-5 补）：不再算随包卸掉的。
        catalog.gone.retain(|_, owner| owner != package);
        catalog.provided.insert(package.to_string());
        for tool in tools {
            catalog.add(package, tool)?;
        }
        Ok(catalog)
    }

    /// 换掉包 `package` 的 MCP 服务 `service` 的工具（施工 X-3 上，`mcp.md`「怎么走」第三条）：交回新的一份，包自己的、别的
    /// 服务的照留；算经提供者登记的（会话下一个回合照现在的换）。查法同 [`Catalog::new`]，但查不过的那一件不登记、别的照登记
    /// （`mcp.md`「出错」）：一并交回没登记的几件。
    pub fn replacing_service(
        &self,
        package: &str,
        service: &str,
        tools: Vec<Arc<dyn Tool>>,
    ) -> (Catalog, Vec<CatalogError>) {
        let prefix = mcp_prefix(service);
        let mut catalog = self.clone();
        catalog.take_out_where(package, |name| name.starts_with(&prefix));
        catalog
            .gone
            .retain(|name, owner| owner != package || !name.starts_with(&prefix));
        catalog.provided.insert(package.to_string());
        let mut skipped = Vec::new();
        for tool in tools {
            if let Err(error) = catalog.add(package, tool) {
                skipped.push(error);
            }
        }
        (catalog, skipped)
    }

    /// 换上这一刻的脚本（施工 X-12 中，`docs/blueprint/skills.md`「脚本」）：原来登记的脚本工具全拿掉，照 `scripts`（包的
    /// 编号和一个脚本的工具，数据根里自己放的是 `~home`）一件件登记；查法同 [`Catalog::new`]，查不过的那一件不登记、别的照登记，
    /// 一并交回没登记的（撞了别的工具的名字的这种）。脚本所在的包算经提供者登记的：会话下一个回合照现在的换。
    pub fn replacing_scripts(
        &self,
        scripts: Vec<(String, Arc<dyn Tool>)>,
    ) -> (Catalog, Vec<CatalogError>) {
        let mut catalog = self.clone();
        for name in std::mem::take(&mut catalog.scripts) {
            catalog.tools.remove(&name);
            catalog.packages.remove(&name);
        }
        catalog.with_scripts(scripts)
    }

    /// 加上这几个脚本，原来登记的脚本不动；查法、交回的同
    /// [`Catalog::replacing_scripts`]：撞了名字的那一件不登记。
    pub fn with_scripts(
        &self,
        scripts: Vec<(String, Arc<dyn Tool>)>,
    ) -> (Catalog, Vec<CatalogError>) {
        let mut catalog = self.clone();
        let mut skipped = Vec::new();
        for (package, tool) in scripts {
            let name = tool.spec().name.clone();
            match catalog.add(&package, tool) {
                Ok(()) => {
                    catalog.scripts.insert(name);
                    catalog.provided.insert(package);
                }
                Err(error) => skipped.push(error),
            }
        }
        (catalog, skipped)
    }

    /// 内置包 `package` 装上了（施工 F-5 中）：换上它的几件，不算经提供者登记的（会话照旧快照的原样留着它们）；以前随它卸掉
    /// 的不再算卸掉。查法同 [`Catalog::new`]。
    ///
    /// # Errors
    ///
    /// 同 [`Catalog::new`]。
    pub fn placing(
        &self,
        package: &str,
        tools: Vec<Arc<dyn Tool>>,
    ) -> Result<Catalog, CatalogError> {
        let mut catalog = self.clone();
        catalog.take_out(package);
        catalog.gone.retain(|_, owner| owner != package);
        for tool in tools {
            catalog.add(package, tool)?;
        }
        Ok(catalog)
    }

    /// 内置包 `package` 卸掉了（施工 F-5 中）：拿掉它的几件，记下它们是随包卸掉的（[`Catalog::gone`]）。
    #[must_use]
    pub fn removing(&self, package: &str) -> Catalog {
        let mut catalog = self.clone();
        let names: Vec<String> = catalog
            .packages
            .iter()
            .filter(|(_, owner)| *owner == package)
            .map(|(name, _)| name.clone())
            .collect();
        catalog.take_out(package);
        catalog
            .gone
            .extend(names.into_iter().map(|name| (name, package.to_string())));
        catalog
    }

    /// 叫 `name` 的那一件是随包卸掉的（施工 F-5 中）。
    pub fn gone(&self, name: &str) -> bool {
        self.gone.contains_key(name)
    }

    /// 拿掉包 `package` 的工具和它们以前的名字。
    fn take_out(&mut self, package: &str) {
        self.take_out_where(package, |_| true);
    }

    /// 拿掉包 `package` 里名字合 `which` 的工具和它们以前的名字。
    fn take_out_where(&mut self, package: &str, which: impl Fn(&str) -> bool) {
        let names: Vec<String> = self
            .packages
            .iter()
            .filter(|(name, owner)| *owner == package && which(name))
            .map(|(name, _)| name.clone())
            .collect();
        for name in &names {
            self.tools.remove(name);
            self.packages.remove(name);
        }
        self.formerly.retain(|_, now| !names.contains(now));
    }

    /// 拿掉包 `package` 的每一件（施工 U23 中中）：脚本、MCP 服务的、提供者登记的都拿掉；随包
    /// 卸掉的记录也拿掉（用过它的会话调到时不报「已卸载」，同没有过：不透露那个包里有过什么）。
    #[must_use]
    pub fn without(&self, package: &str) -> Catalog {
        let mut catalog = self.clone();
        catalog.take_out_where(package, |_| true);
        catalog.gone.retain(|_, owner| owner != package);
        catalog
    }

    /// 登记一件：查过了放进目录，记下它的包。
    fn add(&mut self, package: &str, tool: Arc<dyn Tool>) -> Result<(), CatalogError> {
        let spec = tool.spec();
        let problem = if !name_is_valid(&spec.name) {
            Some(Problem::Name)
        } else if !takes_an_object(spec) {
            Some(Problem::Parameters)
        } else if self.taken(&spec.name) {
            Some(Problem::Duplicate)
        } else {
            None
        };
        if let Some(problem) = problem {
            return Err(CatalogError {
                tool: spec.name.clone(),
                problem,
            });
        }
        for former in tool.formerly() {
            if self.taken(former) {
                return Err(CatalogError {
                    tool: (*former).to_string(),
                    problem: Problem::Duplicate,
                });
            }
            self.formerly
                .insert((*former).to_string(), spec.name.clone());
        }
        self.packages.insert(spec.name.clone(), package.to_string());
        self.tools.insert(spec.name.clone(), tool);
        Ok(())
    }

    /// 叫 `name` 的那一件归哪个包（以前的名字也算）；没有这件的没有。
    pub fn package_of(&self, name: &str) -> Option<&str> {
        let now = self.formerly.get(name).map_or(name, String::as_str);
        self.packages.get(now).map(String::as_str)
    }

    /// 叫 `name` 的那一件是经提供者登记的（施工 O-2 中）；没有这件的不是。
    pub fn provided(&self, name: &str) -> bool {
        self.package_of(name)
            .is_some_and(|package| self.provided.contains(package))
    }

    /// 有工具的几个包，照编号排、不重复。
    pub fn packages(&self) -> impl Iterator<Item = &str> {
        self.packages
            .values()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            .into_iter()
    }

    /// `name` 已经有主了：是一件工具现在的名字，或者以前的名字。
    fn taken(&self, name: &str) -> bool {
        self.tools.contains_key(name) || self.formerly.contains_key(name)
    }

    /// 每件的规格，照名字的先后。
    pub fn specs(&self) -> impl Iterator<Item = &Spec> {
        self.tools.values().map(|tool| tool.spec())
    }

    /// 叫 `name` 的那一件（施工 4-2）；以前叫 `name` 的也算（施工 7-5 再补）：改名以前造的会话，快照里冻着旧名字。
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        let now = self.formerly.get(name).map_or(name, String::as_str);
        self.tools.get(now)
    }
}

impl fmt::Debug for Catalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.tools.keys()).finish()
    }
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let problem = match self.problem {
            Problem::Duplicate => "another tool has the same name",
            Problem::Name => "the name must be 1 to 64 ASCII letters, digits, '_' or '-'",
            Problem::Parameters => "the parameters must be a JSON Schema of type \"object\"",
        };
        write!(f, "tool {:?}: {problem}", self.tool)
    }
}

impl std::error::Error for CatalogError {}

/// 名字合不合写法：英文字母、数字、`_`、`-`，1 到 64 个字符。
fn name_is_valid(name: &str) -> bool {
    (1..=NAME_LIMIT).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

/// 参数格式是不是 `{"type":"object",…}`。
fn takes_an_object(spec: &Spec) -> bool {
    serde_json::from_str::<Value>(spec.parameters.get())
        .ok()
        .as_ref()
        .and_then(|schema| schema.get("type"))
        .and_then(Value::as_str)
        == Some("object")
}

#[cfg(test)]
mod tests;
