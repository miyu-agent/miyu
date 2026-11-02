//! 数据根里每一处在哪（施工 FH-2，设计 `55-数据根仿FHS.md` 第二节）：别的代码只经这些方法拿位置，不自己拼数据根里的路径；
//! 布局要变（施工 FH-3）只改这里。按设计 55 的分块排：配置和凭据（将来的 `etc/`）、你的东西（将来的 `root/`）、机器和包的
//! 状态（`var/lib/`）、能删的缓存（`var/cache/`）、日志（`var/log/`）、暂存（`tmp/`）。FH-2 这一步每个方法照旧交回现在的
//! 位置，一个字节都不挪。
//!
//! 文件名照旧住在各自的模块里（`journal::FILE` 这些），这里只管它在哪个目录。

use std::path::PathBuf;

use super::DataRoot;

/// 状态区（现在的 `state/`）：FH-3 拆进 `var/lib/`、`var/cache/`、`var/log/`，这里只有本文件用。
const STATE: &str = "state";

impl DataRoot {
    // ---- 配置和凭据（设计 55：`etc/`、`etc/private/`） ----

    /// 配置：`config.toml`。
    pub fn config_file(&self) -> PathBuf {
        self.path.join("config.toml")
    }

    /// 项目配置的信任记录：`trust.toml`。
    pub fn trust_file(&self) -> PathBuf {
        self.path.join("trust.toml")
    }

    /// 扩展开没开、批过的能力：`extensions.json`（[`crate::extensions`]）。
    pub fn extensions_file(&self) -> PathBuf {
        self.path.join(crate::extensions::FILE)
    }

    /// 包带的知识库开不开「自动想起」：`kb.toml`（施工 K-8）。
    pub fn knowledge_settings_file(&self) -> PathBuf {
        self.path.join("kb.toml")
    }

    /// 密钥：`secrets.toml`。
    pub fn secrets_file(&self) -> PathBuf {
        self.path.join("secrets.toml")
    }

    /// 访问密码的哈希：`access.json`（[`crate::access`]）。
    pub fn access_file(&self) -> PathBuf {
        self.path.join(crate::access::FILE)
    }

    /// 设备令牌的哈希：`logins.json`（[`crate::logins`]）。
    pub fn logins_file(&self) -> PathBuf {
        self.path.join(crate::logins::FILE)
    }

    /// Windows 沙盒的安装记录、失败原因所在的目录：`state/sandbox/`（施工 5-8；FH-3 拆成凭据和日志两处）。
    pub fn sandbox_install_dir(&self) -> PathBuf {
        self.path.join(STATE).join("sandbox")
    }

    /// 卸掉的出厂包 `id` 的记号：`packages/<编号>.removed`（施工 F-5 上，像 systemd 的 mask）。
    pub fn removed_marker(&self, id: &str) -> PathBuf {
        self.packages().join(format!("{id}.removed"))
    }

    // ---- 你的东西（设计 55：`root/`） ----

    /// 你那一层的根（施工 FH-2）：你的人格、预设、提示词和工具说明的覆盖都在它下面，照名字分（`personas/`、`presets/`、
    /// `prompts/`、`tools/`）；看改动、卸包留下的改动照它找，给人看的位置照它写成相对的。
    pub fn user_layer(&self) -> PathBuf {
        self.path.clone()
    }

    /// 你那一层的人格：`personas/`。
    pub fn personas(&self) -> PathBuf {
        self.path.join("personas")
    }

    /// 你那一层的预设：`presets/`。
    pub fn presets(&self) -> PathBuf {
        self.path.join("presets")
    }

    /// 提示词的覆盖：`prompts/`（施工 OV-1）。
    pub fn prompt_overrides(&self) -> PathBuf {
        self.path.join(crate::resources::PROMPTS)
    }

    /// 工具说明的覆盖：`tools/`（施工 OV-1）。
    pub fn tool_overrides(&self) -> PathBuf {
        self.path.join(crate::overrides::TOOLS)
    }

    /// 你的技能：`skills/`。
    pub fn skills(&self) -> PathBuf {
        self.path.join("skills")
    }

    /// 你的脚本：`scripts/`。
    pub fn scripts(&self) -> PathBuf {
        self.path.join("scripts")
    }

    /// 你的斜杠命令模板：`commands/`。
    pub fn commands(&self) -> PathBuf {
        self.path.join("commands")
    }

    /// 照 XDG 给你的那一处 `name`（[`super::USER_DIRS`] 里的一个）：`<name>/`（施工 Y-11 下）。不建。
    pub fn user_dir(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    /// 你的头像放在哪个目录（施工 AV-1）：文件名是 `avatar.<png|jpg|webp>`。
    pub fn avatar_dir(&self) -> PathBuf {
        self.path.clone()
    }

    /// 跟着人格 `persona` 的记忆日志：`modules/memory/<人格>/`（施工 R-3 上）。
    pub fn memory_log(&self, persona: &str) -> PathBuf {
        self.memory_logs().join(persona)
    }

    /// 放各人格记忆日志的目录：`modules/memory/`。
    pub fn memory_logs(&self) -> PathBuf {
        self.path.join("modules").join("memory")
    }

    /// 你的知识库：`kb/`（施工 K-1 上）。
    pub fn knowledge(&self) -> PathBuf {
        self.path.join("kb")
    }

    /// 项目：`projects.json`（[`crate::projects`]）。
    pub fn projects_file(&self) -> PathBuf {
        self.path.join(crate::projects::FILE)
    }

    /// 定时任务（唤醒表）：`state/schedules.db`。
    pub fn schedules_file(&self) -> PathBuf {
        self.path.join(STATE).join("schedules.db")
    }

    /// 各终端进哪个会话：`terminal.json`（[`crate::terminal`]）。
    pub fn terminal_file(&self) -> PathBuf {
        self.path.join(crate::terminal::FILE)
    }

    /// 脚本的状态（`MIYU_SCRIPT_STATE_DIR` 的根，施工 X-44）：`state/scripts/`，下面是 `[@<场所>/]<来处>/<工具>/`。
    pub fn script_states(&self) -> PathBuf {
        self.path.join(STATE).join("scripts")
    }

    // ---- 机器和包的状态（设计 55：`var/lib/`） ----

    /// 日志：配置、密钥、信任的改动，删掉的会话和一次性调用的用量：`journal.jsonl`（[`crate::journal`]）。
    pub fn journal_file(&self) -> PathBuf {
        self.path.join(crate::journal::FILE)
    }

    /// 运行日志里记忆短号的盐、核心的几样小状态所在的目录：`state/`（[`crate::recall::LogSalt`]）。
    pub fn salt_dir(&self) -> PathBuf {
        self.path.join(STATE)
    }

    /// 路由用出来的东西（学到的上下文窗口、池的指针、供应商改名）：`state/models/`（施工 8-7、8-8、8-34）。
    pub fn model_state(&self) -> PathBuf {
        self.path.join(STATE).join("models")
    }

    /// 包的本地库（装的版本、来源、文件哈希）：`state/packages/.local/`（施工 IN-1）。
    pub fn local_packages(&self) -> PathBuf {
        self.package_states().join(crate::packages::local::LOCAL)
    }

    /// 卸包时留下的人格、预设改动是谁的，记在哪个目录：`state/packages/`，文件名照 `packages::leftovers::FILE`（施工 P-11 下）。
    pub fn leftovers_dir(&self) -> PathBuf {
        self.package_states()
    }

    /// 包 `id` 自己的状态：`state/packages/<编号>/`（`MIYU_PACKAGE_STATE`）。
    pub fn package_state(&self, id: &str) -> PathBuf {
        self.package_states().join(id)
    }

    /// 放各包状态的目录：`state/packages/`。
    pub fn package_states(&self) -> PathBuf {
        self.path.join(STATE).join("packages")
    }

    /// 命令型 MCP 服务 `service` 的工作目录：`state/mcp/<服务>/`（施工 X-3 下）。
    pub fn mcp_workdir(&self, service: &str) -> PathBuf {
        self.mcp_states().join(service)
    }

    /// 放 MCP 服务工作目录、工具目录缓存的目录：`state/mcp/`。
    pub fn mcp_states(&self) -> PathBuf {
        self.path.join(STATE).join("mcp")
    }

    // ---- 能删的缓存（设计 55：`var/cache/`） ----

    /// 用量汇总：`state/usage.db`（[`crate::usage`]）。
    pub fn usage_file(&self) -> PathBuf {
        self.path.join(STATE).join(crate::usage::FILE)
    }

    /// 汇率：`state/rates.db`。
    pub fn rates_file(&self) -> PathBuf {
        self.path.join(STATE).join(crate::usage::rates::FILE)
    }

    /// 核心生成的配置 Schema、参考文件：`state/config/`（施工 8-1）。
    pub fn config_reference(&self) -> PathBuf {
        self.path.join(STATE).join("config")
    }

    /// 拉到的供应商模型列表、目录答的记录：`state/models/`（施工 8-7；FH-3 和 [`DataRoot::model_state`] 分开）。
    pub fn model_cache(&self) -> PathBuf {
        self.path.join(STATE).join("models")
    }

    /// 官方软件源的索引缓存：`state/packages/.sync/`（施工 G-3）。
    pub fn official_feed(&self) -> PathBuf {
        self.package_states().join(".sync")
    }

    /// 配置里加的别的软件源的索引缓存：`state/packages/.sources/`，下面一个源一个目录（施工 U23 下下）。
    pub fn other_feeds(&self) -> PathBuf {
        self.package_states().join(".sources")
    }

    /// 下载的包文件：`state/packages/.cache/`（施工 G-4）。
    pub fn package_downloads(&self) -> PathBuf {
        self.package_states().join(".cache")
    }

    /// MCP 服务 `service` 的工具目录缓存：`state/mcp/<服务>.json`（施工 X-3 上）。
    pub fn mcp_catalog(&self, service: &str) -> PathBuf {
        self.mcp_states().join(format!("{service}.json"))
    }

    /// 包 `package` 登记的工具原文缓存：`state/providers/<包>.json`。
    pub fn provide_cache(&self, package: &str) -> PathBuf {
        self.path
            .join(STATE)
            .join("providers")
            .join(format!("{package}.json"))
    }

    /// 脚本的缓存（`MIYU_SCRIPT_CACHE_DIR` 的根，施工 Y-11 上）：`cache/`，下面是 `[@<场所>/]<来处>/<工具>/`；30 天没碰的清掉
    /// （[`crate::cache`] 只扫这一处）。
    pub fn script_caches(&self) -> PathBuf {
        self.path.join(super::CACHE)
    }

    /// 人格图片的版本、主色缓存所在的目录：`state/`，文件 `personas/pictures.json`（施工 P-7）。
    pub fn persona_picture_cache(&self) -> PathBuf {
        self.path.join(STATE).join("personas")
    }

    /// mermaid 的字体缓存：`cache/mermaid/`。
    pub fn mermaid_cache(&self) -> PathBuf {
        self.path.join(super::CACHE).join("mermaid")
    }

    /// 探到的沙盒能力记在哪个目录：`state/`，文件 `sandbox-probe.json`（施工 5-4 下）。
    pub fn sandbox_probe_dir(&self) -> PathBuf {
        self.path.join(STATE)
    }

    /// 性能测试工具自己的缓存：`state/perf-cache`。
    pub fn perf_cache(&self) -> PathBuf {
        self.path.join(STATE).join("perf-cache")
    }

    // ---- 日志（设计 55：`var/log/`） ----

    /// 运行日志的目录：`state/logs/`（核心的 `core.log`、`memory.jsonl`）。
    pub fn logs(&self) -> PathBuf {
        self.path.join(STATE).join("logs")
    }

    /// 包 `id` 自己写日志的目录：`state/logs/<编号>/`（施工 FH-2下；FH-3 起是 `var/log/<编号>/`，设计 55 第四节的
    /// `MIYU_LOG_DIR`）。不建：用的一方建。核心记的扩展 stderr（[`DataRoot::extension_stderr`]）FH-3 起也落在这个目录里，叫
    /// `stderr`、`stderr.old`，包自己不用这两个名字。
    pub fn package_logs(&self, id: &str) -> PathBuf {
        self.logs().join(id)
    }

    /// 扩展（包 `id` 的实例）的标准错误：`state/logs/<编号>.stderr`（施工 F-10）。
    pub fn extension_stderr(&self, id: &str) -> PathBuf {
        self.logs().join(format!("{id}.stderr"))
    }

    /// MCP 服务 `service` 的标准错误：`state/logs/mcp-<服务名>.stderr`（施工 X-4）。
    pub fn mcp_stderr(&self, service: &str) -> PathBuf {
        self.logs().join(format!("mcp-{service}.stderr"))
    }

    // ---- 暂存（设计 55：`tmp/`） ----

    /// 装 `.miyu` 时解包的暂存：`state/packages/.unpack/`（施工 G-1）。
    pub fn unpack_staging(&self) -> PathBuf {
        self.package_states().join(".unpack")
    }
}
