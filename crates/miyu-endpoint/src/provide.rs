//! `provide {tools}`（施工 O-2 上，`docs/blueprint/providers.md`）：核心拉起的扩展把它现在提供的全部工具登记进核心的工具目录，
//! 归它的包，换掉它上一次登记的；记下这个包现在由这个连接提供（[`Provided`]）。连接断了，工具照旧留在目录里，被调到时回
//! 「暂时不可用」。登记缓存在磁盘上，核心起来、开扩展时先照缓存登记；关掉扩展，它的工具出目录（施工 O-2 中，[`cache`]）。

mod cache;
mod commands;
mod declared;
mod effects;
mod remote;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use miyu_config::package::Capability;
use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::{Access, Role};
use miyu_tool::{Problem, Scope, Spec, Tool, Venues};

use crate::Core;
use crate::extensions::{Instance, State};
use crate::hello::Caller;
use crate::refusal::Refusal;
use crate::reverse::Peer;

#[cfg(test)]
use remote::referenced;
use remote::{RemoteTool, Stores, Texts};

/// 哪个包的进程现在由哪个连接提供。
#[derive(Default)]
pub(crate) struct Provided {
    /// 连接，和登记时那个扩展进程的进程号（施工 XG-1 下：通道照它认子孙；问不出的没有）。
    peers: Mutex<HashMap<Instance, (Peer, Option<u32>)>>,
    /// 握手成了的那几份的连接（施工 J-4）：不管登没登记工具，推通知（`schedule.fired`）照它找。工具调用照旧只认登记了工具的
    /// （[`Provided::peers`]），握了手还没登记的不转过去。
    linked: Mutex<HashMap<Instance, Peer>>,
}

impl Provided {
    /// 现在提供包 `package` 那一份（一份的）的连接；没有的是没有。
    pub(crate) fn peer(&self, package: &str) -> Option<Peer> {
        self.peer_of(&Instance::of(package))
    }

    /// 现在提供 `instance` 的连接；没有的是没有。
    pub(crate) fn peer_of(&self, instance: &Instance) -> Option<Peer> {
        self.peers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(instance)
            .map(|(peer, _)| peer.clone())
    }

    /// 现在提供包 `package` 的那个扩展进程的进程号（施工 XG-1 下）：没有连接、问不出的没有。
    pub(crate) fn pid(&self, package: &str) -> Option<u32> {
        self.peers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&Instance::of(package))
            .and_then(|(_, pid)| *pid)
    }

    /// 记下 `instance` 由 `peer` 提供、扩展进程是 `pid`，换掉以前的。
    fn register(&self, instance: Instance, peer: Peer, pid: Option<u32>) {
        self.peers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(instance, (peer, pid));
    }

    /// 记下 `instance` 握手成了、连接是 `peer`（施工 J-4），换掉以前的。
    pub(crate) fn link(&self, instance: Instance, peer: Peer) {
        self.linked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(instance, peer);
    }

    /// 包 `package` 握了手、还连着的连接（施工 J-4：到点推给建它的扩展），不用登记工具；没连着的没有（它归开机、扩展管理拉起）。
    pub(crate) fn linked(&self, package: &str) -> Option<Peer> {
        self.linked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&Instance::of(package))
            .filter(|peer| !peer.closed())
            .cloned()
    }
}

/// `provide` 的参数，也是登记缓存的写法（施工 O-2 中）。
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProvideParams {
    tools: Vec<ToolParams>,
}

/// 一件工具的规格。
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ToolParams {
    name: String,
    description: String,
    input_schema: Value,
    access: String,
    venues: Vec<String>,
    /// 等它答多久（施工 O-2 下）：毫秒，[`TIMEOUT_MS`] 以内，不写是 [`DEFAULT_TIMEOUT_MS`]。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timeout_ms: Option<u64>,
    /// 给人看的名字，照语言（`zh`、`en`……；施工 9-8 六补）：头在时间线上写它，不写的照工具名。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    display_name: BTreeMap<String, String>,
    /// 给人看的名字后面跟哪一个参数的值（施工 S-6 上）：`input_schema` 的 `properties` 里的一个名字，别的写法 `bad_tool`，
    /// `problem` 是 `subject`。头在时间线上照它在名字后面写那个参数。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subject: Option<String>,
    /// 退役的「要先加载」（施工 L-1 登记、FL-1 拆掉：工具一律全量）：写了的扩展照旧登记得上，值不看，缓存里不写。
    #[serde(default, rename = "need_load", skip_serializing)]
    _retired_need_load: Option<serde::de::IgnoredAny>,
    /// 脚本里能调（施工 M-2，`39-脚本调工具.md` 第三节）：`true` 的开，不写、写 `false` 的关；别的写法 `bad_tool`，`problem` 是
    /// `clicall`。访问类别不是读、联网、执行的，写了也调不成。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    clicall: Option<Value>,
    /// 脚本里调到时交回的结构化结果的样子（施工 M-4，`39-脚本调工具.md` 第九节第 3 条）：一行字，照出厂工具说明文件的写法；
    /// 不写的没有。别的写法、空白的 `bad_tool`，`problem` 是 `result`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    /// 场所插件（施工 O-60，`venues.md`「场所插件」）：写了的，场所会话里只有那个场所开着它才给（`venue.plugins`），本机会话
    /// 照旧。1 到 64 个字，写法同包的编号（小写字母开头，小写字母、数字、`-`）；别的写法 `bad_tool`，`problem` 是 `plugin`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plugin: Option<Value>,
    /// 职能（施工 XG-2 下，`docs/designs/64-工具职能.md` 第四节）：核心要找「哪件是干这个的」时照它认，不照名字。取值同出厂的说明
    /// 文件（`read_file`）；别的写法 `bad_tool`，`problem` 是 `role`。不写的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    role: Option<Value>,
    /// 范围（施工 XG-2 下，设计 64 第五节）：`any`、`local`、`local_main`、`local_main_attended`、`spawn`；只能收窄，`venues` 照旧
    /// 管。别的写法 `bad_tool`，`problem` 是 `scope`。不写的是 `any`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scope: Option<Value>,
    /// 哪几个参数是路径（施工 XG-1 上，`docs/designs/63-扩展工具的效果和声明.md` 第二、三节）：`[{"parameter", "write"?}]`，核心
    /// 照它报要碰的路径、过权限链，回应里的文件效果只收过了链的。写法见 [`declared::paths`]；不对的 `bad_tool`，`problem` 是
    /// `paths`。不写的没有：调用不报路径，文件效果一条都不收。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    paths: Option<Value>,
    /// 只在会话有后台时才给的几个参数（施工 XG-1 上，设计 63 第六节第 1 条）：后台运行关着的会话工具面上去掉。写法见
    /// [`declared::background`]；不对的 `bad_tool`，`problem` 是 `background_parameters`。不写的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    background_parameters: Option<Value>,
    /// 会跑命令（施工 XG-1 下，设计 63 第二节、第六节第 2 条）：`true` 的，说明后面接 `miyu call` 那一段，`tool.call` 带
    /// `channel`，命令里的 `miyu call` 照它连上来调会话的工具。不写、`false` 的不是；别的写法 `bad_tool`，`problem` 是
    /// `runs_commands`；包没批能力 `tools.commands` 的 `needs_approval`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    runs_commands: Option<Value>,
}

/// 不写时等多久：一分钟。
const DEFAULT_TIMEOUT_MS: u64 = 60_000;

/// 能写的时限：一秒到十分钟。
const TIMEOUT_MS: std::ops::RangeInclusive<u64> = 1_000..=600_000;

/// 查过的一件：规格、给哪种会话、等它多久、哪几个参数是路径、哪几个只在有后台时给、会不会跑命令。
struct Checked {
    spec: Spec,
    venues: Venues,
    timeout: Duration,
    display_name: BTreeMap<String, String>,
    subject: Option<String>,
    paths: Vec<declared::PathParameter>,
    background: Vec<String>,
    runs_commands: bool,
}

/// `provide`：见模块的说明。
pub(crate) async fn provide(
    core: &Arc<Core>,
    caller: &Caller,
    params: ProvideParams,
) -> Result<Value, Refusal> {
    let Some(package) = caller.package.as_deref() else {
        return Err(Refusal::NOT_A_PROVIDER);
    };
    let kept = serde_json::to_vec(&params).map_err(|_| Refusal::INTERNAL)?;
    let count = register(core, package, params.tools)?;
    // 扩展进程的进程号（施工 XG-1 下）：拉起以后就记下了（`Starting`），握了手的连接一定有。
    let pid = match core.extensions.status(package).state {
        State::Starting { pid } | State::Running { pid } => pid,
        _ => None,
    };
    core.provided
        .register(Instance::of(package), caller.reverse.clone(), pid);
    cache::write(core, package, &kept);
    tracing::info!(target: "miyu::endpoint", package, tools = count, "provided");
    Ok(json!({ "tools": count }))
}

/// 照规格 `tools` 换掉包 `package` 在目录里的工具，交回几件：`provide`、读缓存共用。两处同时换的一个接一个（[`miyu_tool::Shelf`]）。
fn register(core: &Core, package: &str, tools: Vec<ToolParams>) -> Result<usize, Refusal> {
    let texts = Texts {
        unavailable: core
            .resources
            .core_texts()
            .map(|texts| texts.tool_results.unavailable)
            .map_err(|_| Refusal::INTERNAL)?,
        timed_out: core
            .resources
            .tool_timed_out()
            .map_err(|_| Refusal::INTERNAL)?,
    };
    let count = tools.len();
    let listed = listed(core, package);
    let platforms = crate::venues::platforms_of_package(core, package);
    let declared = Arc::new(crate::sessions::state::Declared::of(core, package));
    let mut all = Vec::with_capacity(count);
    for tool in tools {
        // `mcp__` 开头的名字留给 MCP 服务（施工 X-3 上）：包自己登记的用了，预设会把它归到服务的功能上。
        if tool.name.starts_with(miyu_tool::MCP_PREFIX) {
            return Err(bad_tool(&tool.name, "name"));
        }
        // 写了 `[features]` 的包登记的工具要归它列的某个功能（施工 T-2）：归不上的预设开关不了它。
        if listed
            .as_ref()
            .is_some_and(|listed| !listed.contains(&tool.name))
        {
            return Err(bad_tool(&tool.name, "feature"));
        }
        all.push(checked(tool)?);
    }
    // 会跑命令的要包批过 `tools.commands`（施工 XG-1 下，设计 63 第七节第 3 条：开的时候人批，出厂的算批过）。
    let runs = all.iter().any(|checked| checked.runs_commands);
    if runs && !crate::extensions::held(core, package).contains(&Capability::ToolsCommands) {
        return Err(Refusal::needs_approval(json!([
            Capability::ToolsCommands.as_str()
        ])));
    }
    let commands = match runs {
        true => Some(commands::Commands::load(core)?),
        false => None,
    };
    let mut built: Vec<Arc<dyn Tool>> = Vec::with_capacity(count);
    for checked in all {
        built.push(Arc::new(
            RemoteTool::new(
                checked,
                package,
                Arc::clone(&core.provided),
                texts.clone(),
                Stores {
                    root: core.root.clone(),
                    declared: Arc::clone(&declared),
                },
            )
            .for_platform(platforms.iter())
            .running(commands.clone()),
        ));
    }
    core.tools
        .replace(|catalog| catalog.replacing(package, built))
        .map_err(|error| bad_tool(&error.tool, problem(error.problem)))?;
    Ok(count)
}

/// 包 `package` 的工具归得上的：清单写了 `[features]`、又不是只有一个功能的，是它列的工具名（施工 T-2，归法同
/// `miyu_policy::features::Features::of_tool`）。没写的整个包算一个功能、只写了一个功能的都归它、清单读不成的，没有：不查。
fn listed(core: &Core, package: &str) -> Option<BTreeSet<String>> {
    let packages = core.system_packages();
    let manifest = packages
        .iter()
        .find(|one| one.id == package)?
        .read
        .as_ref()
        .ok()?;
    let features = manifest
        .features
        .as_ref()
        .filter(|features| features.len() != 1)?;
    Some(
        features
            .iter()
            .flat_map(|feature| feature.tools.iter().cloned())
            .collect(),
    )
}

/// 关掉扩展（施工 O-2 中）：包 `package` 的工具出目录，开着的会话下一个回合换掉；缓存留着，再开时照它先登记。
pub(crate) fn withdraw(core: &Core, package: &str) {
    let withdrawn = core
        .tools
        .replace(|catalog| catalog.replacing(package, Vec::new()));
    if let Err(error) = withdrawn {
        // 拿掉不会撞名：只是把这个包的去掉。
        tracing::error!(target: "miyu::endpoint", package, error = %error, "tools not withdrawn");
    }
}

pub(crate) use cache::restore;

/// 查一件的访问类别、给哪种会话、时限；名字、参数格式、撞名由目录查（[`miyu_tool::Catalog::replacing`]）。
fn checked(tool: ToolParams) -> Result<Checked, Refusal> {
    let access = serde_json::from_value::<Access>(Value::String(tool.access.clone()))
        .ok()
        .filter(|access| !matches!(access, Access::Other(_)))
        .ok_or_else(|| bad_tool(&tool.name, "access"))?;
    let mut venues = Venues::default();
    for venue in &tool.venues {
        match venue.as_str() {
            "local" => venues.local = true,
            "private" => venues.private = true,
            "group" => venues.group = true,
            _ => return Err(bad_tool(&tool.name, "venues")),
        }
    }
    if venues == Venues::default() {
        return Err(bad_tool(&tool.name, "venues"));
    }
    let timeout = tool.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    if !TIMEOUT_MS.contains(&timeout) {
        return Err(bad_tool(&tool.name, "timeout"));
    }
    let parameters: RawJson = serde_json::from_str(&tool.input_schema.to_string())
        .map_err(|_| bad_tool(&tool.name, "parameters"))?;
    // 显示名：语言、名字都不能是空白的。
    if tool
        .display_name
        .iter()
        .any(|(language, name)| language.trim().is_empty() || name.trim().is_empty())
    {
        return Err(bad_tool(&tool.name, "display_name"));
    }
    // 跟在名字后面的参数：要是参数格式里有的那一个。
    if let Some(subject) = &tool.subject {
        let declared = tool.input_schema["properties"]
            .as_object()
            .is_some_and(|properties| properties.contains_key(subject));
        if !declared {
            return Err(bad_tool(&tool.name, "subject"));
        }
    }
    let clicall = match &tool.clicall {
        None => false,
        Some(Value::Bool(on)) => *on,
        Some(_) => return Err(bad_tool(&tool.name, "clicall")),
    };
    let result = match &tool.result {
        None => None,
        Some(Value::String(shape)) if !shape.trim().is_empty() => Some(shape.clone()),
        Some(_) => return Err(bad_tool(&tool.name, "result")),
    };
    let plugin = match &tool.plugin {
        None => None,
        Some(Value::String(name)) if plugin_name(name) => Some(name.clone()),
        Some(_) => return Err(bad_tool(&tool.name, "plugin")),
    };
    let role = match &tool.role {
        None => None,
        Some(value) => Some(
            serde_json::from_value::<Role>(value.clone())
                .map_err(|_| bad_tool(&tool.name, "role"))?,
        ),
    };
    let scope = match &tool.scope {
        None => Scope::Any,
        Some(value) => serde_json::from_value::<Scope>(value.clone())
            .map_err(|_| bad_tool(&tool.name, "scope"))?,
    };
    let paths = declared::paths(&tool.name, tool.paths.as_ref(), &tool.input_schema, &access)?;
    let background = declared::background(
        &tool.name,
        tool.background_parameters.as_ref(),
        &tool.input_schema,
    )?;
    let runs_commands = match &tool.runs_commands {
        None => false,
        Some(Value::Bool(runs)) => *runs,
        Some(_) => return Err(bad_tool(&tool.name, "runs_commands")),
    };
    Ok(Checked {
        spec: Spec {
            name: tool.name,
            description: tool.description,
            parameters,
            access,
            clicall,
            result,
            plugin,
            role,
            scope,
        },
        venues,
        timeout: Duration::from_millis(timeout),
        display_name: tool.display_name,
        subject: tool.subject,
        paths,
        background,
        runs_commands,
    })
}

/// 场所插件的名字（施工 O-60）：同包的编号的写法，小写字母开头，小写字母、数字、`-`，1 到 64 个字。
pub(crate) fn plugin_name(name: &str) -> bool {
    let mut chars = name.chars();
    name.len() <= 64
        && chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// 目录查出来的那一条，写成协议上的字。
fn problem(problem: Problem) -> &'static str {
    match problem {
        Problem::Duplicate => "duplicate",
        Problem::Name => "name",
        Problem::Parameters => "parameters",
    }
}

/// `bad_tool`：哪一件、哪一条。
fn bad_tool(tool: &str, problem: &str) -> Refusal {
    Refusal::bad_tool(tool, problem)
}

#[cfg(test)]
mod tests;
