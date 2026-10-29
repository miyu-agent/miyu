//! 核心进程（`docs/designs/12-进程形态与分发.md` 第二节，施工 3-9 上）：`miyu core`，由头拉起，平时不用人敲。
//!
//! 起来的先后：
//!
//! 1. 找数据根，建骨架；
//! 2. 拿单实例锁：已经有一个核心在跑的，说一声 `running` 就走；先拿锁再装日志，免得两个核心写同一份；
//! 3. 装运行日志 `state/logs/core.log`，记一条「起来了」：版本、进程号、数据根、和 UTC 差多少；
//! 4. 工作区，没有就建；资源目录；读配置、照 `log.level` 换运行日志的级别，照配置清单生成两份 JSON
//!    Schema 和参考文件（[`settings`]，施工 8-1、8-2）；供应商的档案、认原厂的表，造会话的路由（[`models`]，施工 8-6、8-7）；开始监视配置文件，配置换了当场换级别、重写
//!    生成的文件（施工 8-4）；
//! 5. 换本机令牌、在套接字上等连接（施工 3-8 下）；找沙盒的助手、探一次，只记日志（施工 5-1）；照编进来的可选
//!    软件包往查询表里登记（[`packages::register`]，cargo 开关 `mermaid`、`net`，施工 W-4、W-7），交给 `Core`；清掉
//!    分块上传留下的暂存（[`packages::clear_uploads`]，施工 W-5）；
//! 6. 往标准输出写一行 `ready`：拉起它的头等着这一行；接着照开关拉起开着的扩展（施工 9-4 上）；在后台更新 models.dev 的
//!    目录（施工 8-7；读目录、用出来的在第 4 步造好路由就在阻塞线程里开始了，读完再答要它的，施工 V-2 下）；在后台清一次
//!    回收处（施工 3-8 三补，`trash.rs`）；开始自己记内存账（施工 V-4，[`memlog`]）；一阵请求过去、安静下来以后还内存（施工
//!    V-5 中，`settle.rs`）。
//!
//! 之后 [`serve()`] 一个个接连接：没有连接、也没有在跑的回合，空闲够久了就退出；收到停的信号，先让在跑的
//! 会话有计划地停下再退出。起不来的，把原因写成那一行（`error …`）交给头。

mod cache;
pub mod embed;
mod kit;
pub mod memlog;
pub mod models;
pub mod packages;
mod pending;
mod sandbox;
mod serve;
pub mod settings;
mod settle;
mod start;
mod trash;

pub use kit::Kit;
pub use serve::{Stopped, idle_limit, serve, serve_then};

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use miyu_endpoint::Core;
use miyu_ipc::{Lock, OpenError, Ready};
use miyu_store::env::Env;
use miyu_store::packages::Found;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_tool::Catalog;

/// 运行日志的目标。
const TARGET: &str = "miyu::core";

/// 核心的运行时开几个线程：接连接、会话、请求都是等 I/O，两个够用，占用也低。
const WORKERS: usize = 2;

/// `miyu core` 的参数。
#[derive(Debug, Clone)]
pub struct Options {
    /// 空闲多久退出：有的盖过系统配置的 `core.idle_exit`（开发、测试用，`miyu core --idle-seconds`）；没有的照配置（施工 Z-1 上）。
    pub idle: Option<Duration>,
}

/// 跑核心进程，交回退出码。
pub fn main(options: Options) -> ExitCode {
    // 堆先调好（23 F3，施工 V-2 再补）：arena 的个数要在起别的线程之前限。
    let heap = miyu_heap::tune();
    let env = Env::current();
    let root = match DataRoot::locate(&env) {
        Ok(root) => root,
        Err(error) => return failed("data_root", error.to_string()),
    };
    if let Err(error) = root.prepare() {
        return failed("data_root", error.to_string());
    }
    let lock = match Lock::acquire(&root) {
        Ok(lock) => lock,
        Err(OpenError::Running) => {
            say(&Ready::Running);
            return ExitCode::SUCCESS;
        }
        Err(error) => return failed("lock", error.to_string()),
    };
    // 装上时照 `MIYU_LOG`（没设、读不懂的是 INFO），读完配置再照 `log.level` 换（施工 8-2）。
    let level = miyu_log::level(std::env::var("MIYU_LOG").ok().as_deref());
    let logs = root.state().join("logs");
    let log = match miyu_log::install(&logs, "core", level.filter, env.home.as_deref()) {
        Ok(guard) => guard,
        Err(error) => return failed("log", error.to_string()),
    };
    tracing::info!(
        target: TARGET,
        version = env!("CARGO_PKG_VERSION"),
        pid = std::process::id(),
        root = %root.path().display(),
        tz = %miyu_log::utc_offset(),
        "starting"
    );
    tracing::debug!(
        target: TARGET,
        tuned = heap,
        arenas = miyu_heap::ARENAS,
        mmap_from = miyu_heap::MMAP_FROM,
        "heap tuned"
    );
    // 核心没了，它起的子进程跟着结束（施工 7-8，`core.md`「起来的先后」第 4 条）：Windows 上核心进作业对象，别的平台什么都
    // 不做（Unix 上每条命令的组里有看门的）。进不去照样起来。
    if let Err(error) = miyu_sandbox::lifeline::bind_children() {
        tracing::warn!(target: TARGET, error = %error, "children not bound");
    }
    let resources = match ResourceRoot::locate(&env) {
        Ok(resources) => resources,
        Err(error) => return failed("resources", error.to_string()),
    };
    // 系统的证书也在旁边先读（施工 V-5 上）：整个进程只读这一次（`miyu_http::tls`），后面造 HTTP 客户端时多半已经读好了。
    drop(std::thread::spawn(|| drop(miyu_http::tls())));
    // 目录最先读（施工 V-2 下补）：和下面读清单、配置、生成文件并着走。
    let catalog = models::read_early(
        resources
            .catalog_snapshot()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default(),
        models::cache(&env),
        env.exe.clone(),
    );
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(WORKERS)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return failed("runtime", error.to_string()),
    };
    let outcome = runtime.block_on(start::start(
        env,
        (root, resources, lock),
        options,
        (&level, &log),
        catalog,
    ));
    drop(log);
    outcome
}

/// 这一份核心编进来的内置包（施工 F-2，设计 `30-插件框架.md` 第二节）：清单是内置包、编号不在这里的，照读坏了的清单报
/// （`miyu_endpoint::packages::compiled`）。画 mermaid、联网照 cargo 开关。
pub fn built_in() -> Vec<&'static str> {
    // 照开关挑，不用 `#[cfg]` 掉 `push`：两个开关都关着时 `mut` 就成了多余的，clippy 报（不带默认开关单独编它时就是这样）。
    miyu_basesystem::PACKAGES
        .iter()
        .copied()
        .map(Some)
        .chain([
            Some(miyu_memory::PACKAGE),
            Some(miyu_goal::PACKAGE),
            Some(miyu_schedule::SCHEDULE),
            Some(miyu_schedule::LOOP),
            Some(miyu_knowledge::PACKAGE),
            Some(ROLEPLAY),
            Some(miyu_policy::shown::PACKAGE),
            cfg!(feature = "mermaid").then_some("mermaid"),
            cfg!(feature = "net").then_some("net"),
        ])
        .flatten()
        .collect()
}

/// 人设遵循提醒这个内置包的编号（施工 F-2）：代码在策略、会话两层，核心这里只认编号。
const ROLEPLAY: &str = "roleplay";

/// 工具目录：核心起来时登记一次，登记完就冻结（`05-内核接口.md` 第八节）。施工 4-4 起登记基础系统，施工 R-3 中起登记记忆
/// 这个软件包，工具的字从资源目录 `resources` 读。施工 F-2 起照清单 `found`：没装的内置包（没有读成了的清单）不登记。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对；登记时查不过（重名、名字或参数格式不合写法）。
pub fn tools(resources: &ResourceRoot, found: &[Found]) -> Result<Catalog, String> {
    tools_in(resources, found, &Kit::new(resources))
}

/// 同 [`tools`]，有家底的内置包照 `kit`（施工 N-2 下）：核心起来时造一份，和端口共用。
///
/// # Errors
///
/// 同 [`tools`]。
pub fn tools_in(resources: &ResourceRoot, found: &[Found], kit: &Kit) -> Result<Catalog, String> {
    // 照软件包登记（施工 P-2 中）：预设照包开关。
    Catalog::in_packages(groups(resources, found, kit)?).map_err(|error| error.to_string())
}

/// 一个编进来的内置包的工具：包的编号和它的几件。
type Builtin = (&'static str, Vec<Arc<dyn miyu_tool::Tool>>);

/// 装了的内置包的工具：包的编号和它的几件（施工 F-5 中从 [`tools`] 拆出来：装卸以后端点经 [`BuiltinTools`] 重新要）。
///
/// # Errors
///
/// 同 [`tools`]：哪一份字读不出来、写法不对。
fn groups(resources: &ResourceRoot, found: &[Found], kit: &Kit) -> Result<Vec<Builtin>, String> {
    let installed = |id| miyu_endpoint::packages::is_installed(found, id);
    let mut groups = Vec::new();
    // 基础系统拆成的包（施工 BS-1 上中，设计 44）：装了哪个登记哪个。
    for (package, tools) in
        miyu_basesystem::packages(resources.path()).map_err(|error| error.to_string())?
    {
        if installed(package) {
            groups.push((package, tools));
        }
    }
    // 记忆这个软件包的三件（施工 R-3 中，`memory.md`「工具」）：工具面上只给本机的主会话。
    if installed(miyu_memory::PACKAGE) {
        let memory = miyu_memory::tools(resources.path()).map_err(|error| error.to_string())?;
        groups.push((miyu_memory::PACKAGE, memory));
    }
    // 长期目标的工具（施工 X-8 上，`goal.md`）：工具面上只给本机的主会话。
    if installed(miyu_goal::PACKAGE) {
        let goal = miyu_goal::tools(resources.path()).map_err(|error| error.to_string())?;
        groups.push((miyu_goal::PACKAGE, goal));
    }
    // 定时任务、循环两个软件包（施工 J-2，设计 45）：`cron_*` 三件、`schedule_wakeup`，预设分开开关。
    if installed(miyu_schedule::SCHEDULE) {
        let schedule =
            miyu_schedule::schedule_tools(resources.path()).map_err(|error| error.to_string())?;
        groups.push((miyu_schedule::SCHEDULE, schedule));
    }
    if installed(miyu_schedule::LOOP) {
        let looping =
            miyu_schedule::loop_tools(resources.path()).map_err(|error| error.to_string())?;
        groups.push((miyu_schedule::LOOP, looping));
    }
    // 知识库这个软件包的两件（施工 K-1 下，`knowledge.md`「她的工具」）：工具面上只给查得到至少一个库的会话。
    if installed(miyu_knowledge::PACKAGE) {
        let knowledge =
            miyu_knowledge::tools(resources.path()).map_err(|error| error.to_string())?;
        groups.push((miyu_knowledge::PACKAGE, knowledge));
    }
    // 联网这个软件包的工具（施工 N-1、N-2，设计 36）：`web_fetch`、`web_search`，只读，哪种会话都有；家底照 `kit`。
    #[cfg(feature = "net")]
    if installed(miyu_net::PACKAGE) {
        let net = kit
            .net_tools(resources)
            .map_err(|error| error.to_string())?;
        groups.push((miyu_net::PACKAGE, net));
    }
    #[cfg(not(feature = "net"))]
    let _ = kit;
    Ok(groups)
}

/// 交给端点的内置包工具的端口（施工 F-5 中，`miyu_endpoint::builtins`）：装卸以后照清单重新登记，工具的字从 `resources` 读；
/// 本机的向量模型照核心起来时的环境 `env` 找小程序（施工 F-5 再补）。
pub fn builtin_tools(
    resources: &ResourceRoot,
    env: &Env,
) -> Arc<dyn miyu_endpoint::builtins::Builtins> {
    builtin_tools_in(resources, env, Kit::new(resources))
}

/// 同 [`builtin_tools`]，有家底的内置包照 `kit`（施工 N-2 下）：和起来时登记工具的那一份共用。
pub fn builtin_tools_in(
    resources: &ResourceRoot,
    env: &Env,
    kit: Kit,
) -> Arc<dyn miyu_endpoint::builtins::Builtins> {
    Arc::new(BuiltinTools {
        resources: resources.clone(),
        env: env.clone(),
        kit,
    })
}

/// [`builtin_tools`] 那一个。
struct BuiltinTools {
    resources: ResourceRoot,
    env: Env,
    kit: Kit,
}

impl miyu_endpoint::builtins::Builtins for BuiltinTools {
    fn tools(&self, found: &[Found]) -> Result<Vec<miyu_endpoint::builtins::Group>, String> {
        Ok(groups(&self.resources, found, &self.kit)?
            .into_iter()
            .map(|(id, tools)| (id.to_string(), tools))
            .collect())
    }

    /// 和起来时同一套拼法（施工 F-5 补）：核心登记的、包声明的，没装的内置包替它声明的那几项设置页不画。
    fn settings(&self, found: &mut [Found]) -> Vec<miyu_config::Item> {
        settings::Packaged::of(found).all()
    }

    /// 和起来时同一套拼法（施工 F-5 再补），不记「没装」的那一行。
    fn embed(&self, found: &[&Found]) -> Option<miyu_session::EmbedSetup> {
        embed::find(&self.env, found).ok()
    }

    /// 脚本照基础系统的造法（施工 X-12 中），给她看的字照资源目录。
    fn script(
        &self,
        head: &miyu_policy::script::ScriptHead,
        file: std::path::PathBuf,
        source: &str,
    ) -> Result<Arc<dyn miyu_tool::Tool>, String> {
        miyu_basesystem::script(self.resources.path(), head, file, source)
            .map_err(|error| error.to_string())
    }

    /// 搜索来源冷却着的那一家（施工 N-2 下）：照这个核心的那一份家底。
    fn option_state(
        &self,
        key: &str,
        option: &str,
        values: &miyu_config::Values,
    ) -> Option<miyu_endpoint::builtins::OptionState> {
        self.kit.option_state(key, option, values)
    }
}

/// 核心这时读成了的清单（施工 F-5 补）：配置清单变了重写生成的文件时照它找包的配置项的字。核心没了的交回空的。
fn manifests_of(core: &Arc<Core>) -> settings::Manifests {
    let core = Arc::downgrade(core);
    Box::new(move || {
        core.upgrade()
            .map(|core| settings::manifests(&core.system_packages()))
            .unwrap_or_default()
    })
}

/// 读两层清单（施工 9-1 上），再标出这一份核心没编进来的内置包（施工 F-2）。
fn load_packages(resources: &ResourceRoot, root: &DataRoot) -> Vec<Found> {
    let mut found = miyu_endpoint::packages::load(resources, root);
    miyu_endpoint::packages::compiled(&mut found, &built_in());
    found
}

/// 运行中配置换了当场生效要的（施工 8-4）：换运行日志级别的把手，核心这边的系统语言；起来时读的软件包清单（施工 9-1
/// 下）：交给核心。
struct Live {
    levels: miyu_log::Levels,
    locale: Option<String>,
    found: Vec<miyu_store::packages::Found>,
}

/// 后半段，在运行时里：在套接字上等连接，说「好了」，接连接，直到停下。`env` 是起来时读的那一份环境快照，`config` 是
/// 起来时读的配置和当场生效要的。开始监视配置文件、跟着配置换级别和重写生成的文件（施工 8-4）在说「好了」之前。
async fn run(
    env: Env,
    (root, resources, opened): (DataRoot, ResourceRoot, miyu_ipc::Opened),
    options: Options,
    (config, live): (miyu_endpoint::config::Config, Live),
    catalog: models::Early,
) -> ExitCode {
    let routes = match models::prepare(&resources, Some(root.state().join("models"))) {
        Ok(routes) => routes,
        Err(error) => return failed("models", error),
    };
    let model_data = Arc::clone(&routes.data);
    // 造好路由就放行目录（施工 V-2 下）：读在找到资源目录时就开始了（施工 V-2 下补），这里等它读完、读用出来的，不挡
    // `ready`。
    let cache = catalog.cache();
    let reading = models::begin(
        Arc::clone(&model_data),
        catalog,
        Some(root.state().join("models")),
    );
    let sandbox = sandbox::probe(env.exe.as_deref(), Some(&root.state()));
    let sandbox_cache = sandbox::cache(&env, std::env::var_os("CARGO_HOME"));
    let embedder = embed::setup(&env, &live.found);
    let kit = Kit::new(&resources);
    let tools = match tools_in(&resources, &live.found, &kit) {
        Ok(tools) => tools,
        Err(error) => return failed("tools", error),
    };
    let (trashed, cached) = (root.clone(), root.clone());
    let logs = root.state().join("logs");
    let (generated, words) = (root.clone(), resources.clone());
    let queries = packages::register(&resources, &root);
    #[cfg(feature = "net")]
    let searching = kit.clone();
    let builtins = builtin_tools_in(&resources, &env, kit);
    packages::clear_uploads(&root);
    let mut core = Core::new(
        root,
        resources,
        Arc::new(routes),
        tools,
        env.home,
        opened.token,
    )
    .with_sandbox(sandbox)
    .with_config(config)
    .with_packages(live.found)
    .with_built_in(built_in())
    .with_builtins(builtins)
    .with_model_data(Arc::clone(&model_data))
    .with_queries(queries);
    if let Some((cache, cargo_home)) = sandbox_cache {
        core = core.with_sandbox_cache(cache, cargo_home);
    }
    core = core.with_vectors(embedder);
    let core = Arc::new(core);
    // 金额换算要的汇率（施工 UC-1）：照环境的代理上网取，同拉目录。造不出客户端的不换算，照分币种显示。
    match miyu_http::fetcher(miyu_http::Proxy::FromEnvironment) {
        Ok(client) => core.install_rates(Arc::new(miyu_session::rates::HttpRates::new(
            client,
            tokio::runtime::Handle::current(),
        ))),
        Err(error) => tracing::warn!(target: TARGET, error = %error, "rates client not made"),
    }
    // 整理记忆跑在整理会话里（施工 D-1 上）：会话表接上记忆。
    core.give_dream_sessions();
    #[cfg(feature = "net")]
    searching.attach(&core);
    tokio::spawn(settings::follow(
        core.config_now(),
        live.levels,
        generated,
        words,
        live.locale,
        manifests_of(&core),
    ));
    models::follow_cooldown(core.config_now(), Arc::clone(&model_data));
    // 监视配置文件（第七条）：拿着它一直到停，丢掉就不看了。
    let _watching = core.watch_config();
    say(&Ready::Ready);
    // 照开关拉起开着的扩展（施工 9-4 上）：写了 `ready` 以后，拉起它的头不等它们。
    core.start_extensions();
    // 配置里的 MCP 服务跟着配置换（施工 F-9 下）：手改了配置，当场起停。
    let _home_mcp = core.follow_home_mcp();
    // 你的脚本跟着变（施工 X-12 补）：放进去、改了、删了，当场再登记一遍。
    let _home_scripts = core.watch_scripts();
    // 池的成员下架了的，路由确认以后从池里拿掉（施工 8-23）。
    core.start_retirement();
    // 唤醒表（施工 J-1 上）：开上已有的、跳过核心停着时错过的，睡到点、到点交进会话。
    let _schedules = core.start_schedules();
    // 自己记内存账（施工 V-4，23 F4）：采到核心没了；量不出来的平台不起。
    let sampling = memlog::spawn(&core, &logs, memlog::Timing::default());
    // 一阵请求过去、安静下来以后把堆里空着的还给系统（施工 V-5 中，[`settle`]），再看内存涨得快不快（施工 V-6 下）：没请求时
    // 不醒。
    let ledger = sampling
        .as_ref()
        .map(|sampling| Arc::clone(&sampling.ledger));
    let _settling = settle::spawn(
        &core.activity(),
        settle::QUIET,
        miyu_heap::trim,
        memlog::on_settled(ledger),
    );
    // 写了 `ready` 以后在后台更新目录、拉图标（施工 8-7，「起草时定的」第 13 条）；读在上面已经开始了（施工 V-2 下）。
    models::follow(reading, cache, models::catalog_settings(core.config_now()));
    let purging = trash::purge(trashed);
    // 清一次每个账号的缓存（施工 Y-11 上）：同回收处，走之前等它清完。
    let sweeping = cache::sweep(cached);
    let (fixed, configured) = (options.idle, Arc::clone(&core));
    let idle = move || fixed.or_else(|| serve::idle_limit(&configured));
    // 走之前的收尾在放锁之前做完（施工 TMP-1 补2）。
    let closing = pending::closing(purging, sweeping);
    serve_then(opened.listener, core, idle, serve::signal(), closing).await;
    ExitCode::SUCCESS
}

/// 起不来：原因写成那一行交给头，头印给人看；运行日志（装上了的话）只记没过的是哪一步 `stage`，原因是给人看的
/// 中文，不进日志（施工 4-9 再补四上）。调它的地方接着就走、放锁，所以先等另起线程写盘的写完（施工 TMP-1 补2）。
fn failed(stage: &'static str, reason: String) -> ExitCode {
    pending::finish();
    tracing::warn!(target: TARGET, stage, "not started");
    say(&Ready::Failed(reason));
    ExitCode::FAILURE
}

/// 往标准输出写那一行：拉起核心的头在管道的另一头等着。
fn say(ready: &Ready) {
    let mut out = io::stdout();
    if let Err(error) = out
        .write_all(ready.line().as_bytes())
        .and_then(|()| out.flush())
    {
        tracing::warn!(target: TARGET, error = %error, "ready line not written");
    }
}
