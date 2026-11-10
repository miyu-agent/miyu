//! 数据根里每一处在哪（施工 FH-2 收到这里，施工 FH-3 换成设计 `55-数据根仿FHS.md` 第二节的布局）：别的代码只经这些方法拿位置，
//! 不自己拼数据根里的路径。按设计 55 的分块排：配置和凭据 `etc/`、`etc/private/`，你的东西 `root/`，机器和包的状态
//! `var/lib/`，能删的缓存 `var/cache/`，日志 `var/log/`，暂存 `tmp/`、`var/tmp/`；包各占一个 `<编号>/`。
//!
//! 文件名照旧住在各自的模块里（`journal::FILE` 这些），这里只管它在哪个目录。

use std::path::PathBuf;

use super::DataRoot;

impl DataRoot {
    // ---- 配置和凭据（设计 55：`etc/`、`etc/private/`） ----

    /// 配置：`etc/config.toml`。
    pub fn config_file(&self) -> PathBuf {
        self.etc().join("config.toml")
    }

    /// 项目配置的信任记录：`etc/trust.toml`。
    pub fn trust_file(&self) -> PathBuf {
        self.etc().join("trust.toml")
    }

    /// 扩展开没开、批过的能力：`etc/extensions.json`（[`crate::extensions`]）。
    pub fn extensions_file(&self) -> PathBuf {
        self.etc().join(crate::extensions::FILE)
    }

    /// 包带的知识库开不开「自动想起」：`etc/kb.toml`（施工 K-8）。
    pub fn knowledge_settings_file(&self) -> PathBuf {
        self.etc().join("kb.toml")
    }

    /// 密钥：`etc/private/secrets.toml`，0600。
    pub fn secrets_file(&self) -> PathBuf {
        self.private().join("secrets.toml")
    }

    /// 访问密码的哈希：`etc/private/access.json`（[`crate::access`]）。
    pub fn access_file(&self) -> PathBuf {
        self.private().join(crate::access::FILE)
    }

    /// 设备令牌的哈希：`etc/private/logins.json`（[`crate::logins`]）。
    pub fn logins_file(&self) -> PathBuf {
        self.private().join(crate::logins::FILE)
    }

    /// Windows 沙盒用户的安装记录（含 DPAPI 加密的密码，施工 5-8）：`etc/private/sandbox-windows.json`。
    pub fn sandbox_record_file(&self) -> PathBuf {
        self.private().join("sandbox-windows.json")
    }

    /// Windows 沙盒安装失败的原因：`var/log/sandbox/windows-error.json`。
    pub fn sandbox_report_file(&self) -> PathBuf {
        self.var_log().join("sandbox").join("windows-error.json")
    }

    /// 卸掉的出厂包的记号放在哪：`etc/removed/`，一个包一个空文件 `<编号>`（施工 F-5 上，像 systemd 的 mask）。
    pub fn removed_markers(&self) -> PathBuf {
        self.etc().join(super::REMOVED)
    }

    /// 卸包留下的设置项的记号放在哪：`etc/leftover/`，一个包一份 `<编号>.toml`（施工 PK-5补，[`crate::packages::settings_left`]）。
    pub fn leftover_settings_dir(&self) -> PathBuf {
        self.etc().join("leftover")
    }

    /// 你装的包 `id` 不带 `purge` 卸掉、`etc/config.toml` 里还留着它的设置项的记号：`etc/leftover/<编号>.toml`，写着它的前缀
    /// （施工 PK-5补）。
    pub fn leftover_settings(&self, id: &str) -> PathBuf {
        self.leftover_settings_dir().join(format!("{id}.toml"))
    }

    // ---- 你的东西（设计 55：`root/`） ----

    /// 你那一层的根（施工 FH-2）：你的人格、预设、提示词和工具说明的覆盖都在它下面，照名字分（`personas/`、`presets/`、
    /// `prompts/`、`tools/`）；看改动、卸包留下的改动照它找，给人看的位置照它写成相对的。
    pub fn user_layer(&self) -> PathBuf {
        self.user()
    }

    /// 你那一层的人格：`root/personas/`。
    pub fn personas(&self) -> PathBuf {
        self.user().join("personas")
    }

    /// 你那一层的预设：`root/presets/`。
    pub fn presets(&self) -> PathBuf {
        self.user().join("presets")
    }

    /// 提示词的覆盖：`root/prompts/`（施工 OV-1）。
    pub fn prompt_overrides(&self) -> PathBuf {
        self.user().join(crate::resources::PROMPTS)
    }

    /// 工具说明的覆盖：`root/tools/`（施工 OV-1）。
    pub fn tool_overrides(&self) -> PathBuf {
        self.user().join(crate::overrides::TOOLS)
    }

    /// 你的技能：`root/skills/`。
    pub fn skills(&self) -> PathBuf {
        self.user().join("skills")
    }

    /// 你的脚本：`root/scripts/`。
    pub fn scripts(&self) -> PathBuf {
        self.user().join("scripts")
    }

    /// 你的斜杠命令模板：`root/commands/`。
    pub fn commands(&self) -> PathBuf {
        self.user().join("commands")
    }

    /// 照 XDG 给你的那一处 `name`（[`super::USER_DIRS`] 里的一个）：`root/<name>/`（施工 Y-11 下）。不建。
    pub fn user_dir(&self, name: &str) -> PathBuf {
        self.user().join(name)
    }

    /// 你的头像放在哪个目录（施工 AV-1）：文件名是 `avatar.<png|jpg|webp>`。
    pub fn avatar_dir(&self) -> PathBuf {
        self.user()
    }

    /// 跟着人格 `persona` 的记忆日志：`root/memory/<人格>/`（施工 R-3 上）。
    pub fn memory_log(&self, persona: &str) -> PathBuf {
        self.memory_logs().join(persona)
    }

    /// 放各人格记忆日志的目录：`root/memory/`。
    pub fn memory_logs(&self) -> PathBuf {
        self.user().join("memory")
    }

    /// 你的知识库：`root/kb/`（施工 K-1 上）。
    pub fn knowledge(&self) -> PathBuf {
        self.user().join("kb")
    }

    /// 项目：`root/projects.json`（[`crate::projects`]）。
    pub fn projects_file(&self) -> PathBuf {
        self.user().join(crate::projects::FILE)
    }

    /// 定时任务（唤醒表）：`root/schedules.db`。
    pub fn schedules_file(&self) -> PathBuf {
        self.user().join("schedules.db")
    }

    /// 各终端进哪个会话：`root/state/terminal.json`（[`crate::terminal`]）。
    pub fn terminal_file(&self) -> PathBuf {
        self.user().join("state").join(crate::terminal::FILE)
    }

    /// 脚本的状态（`MIYU_SCRIPT_STATE_DIR` 的根，施工 X-44）：`root/state/scripts/`，下面是 `[@<场所>/]<来处>/<工具>/`。
    pub fn script_states(&self) -> PathBuf {
        self.user().join("state").join("scripts")
    }

    // ---- 机器和包的状态（设计 55：`var/lib/`） ----

    /// 日志：配置、密钥、信任的改动，删掉的会话和一次性调用的用量：`var/lib/journal.jsonl`（[`crate::journal`]）。
    pub fn journal_file(&self) -> PathBuf {
        self.var_lib().join(crate::journal::FILE)
    }

    /// 运行日志里记忆短号的盐、核心的几样小状态所在的目录：`var/lib/`（[`crate::recall::LogSalt`]）。
    pub fn salt_dir(&self) -> PathBuf {
        self.var_lib()
    }

    /// 路由用出来的东西（学到的上下文窗口、池的指针、供应商改名）：`var/lib/models/`（施工 8-7、8-8、8-34）。
    pub fn model_state(&self) -> PathBuf {
        self.var_lib().join("models")
    }

    /// 包的本地库（装的版本、来源、文件哈希）：`var/lib/packages/`（施工 IN-1）。
    pub fn local_packages(&self) -> PathBuf {
        self.var_lib().join(crate::packages::local::LOCAL)
    }

    /// 卸包时留下的人格、预设改动是谁的，记在哪个目录：`var/lib/packages/`，文件名照 `packages::leftovers::FILE`（施工 P-11 下）。
    pub fn leftovers_dir(&self) -> PathBuf {
        self.local_packages()
    }

    /// 包 `id` 自己的状态：`var/lib/<编号>/`（`MIYU_PACKAGE_STATE`）。
    pub fn package_state(&self, id: &str) -> PathBuf {
        self.var_lib().join(id)
    }

    /// 放各包状态的目录：`var/lib/`（核心自己用的几个名字保留，包不能用作编号，[`super::RESERVED`]）。
    pub fn package_states(&self) -> PathBuf {
        self.var_lib()
    }

    /// 命令型 MCP 服务 `service` 的工作目录：`var/lib/mcp/<服务>/`（施工 X-3 下）。
    pub fn mcp_workdir(&self, service: &str) -> PathBuf {
        self.var_lib().join("mcp").join(service)
    }

    /// 各个包自己存过哪些 blob 的目录：`var/lib/blob-grants/`（施工 BG-1，[`crate::blob::grants`]）。
    pub fn blob_grants_dir(&self) -> PathBuf {
        self.var_lib().join("blob-grants")
    }

    /// 包 `package` 自己存过哪些 blob 的那一份：`var/lib/blob-grants/<包>`，一行一个哈希。
    pub fn blob_grants(&self, package: &str) -> PathBuf {
        self.blob_grants_dir().join(package)
    }

    // ---- 能删的缓存（设计 55：`var/cache/`） ----

    /// 用量汇总：`var/cache/usage.db`（[`crate::usage`]）。
    pub fn usage_file(&self) -> PathBuf {
        self.var_cache().join(crate::usage::FILE)
    }

    /// 汇率：`var/cache/rates.db`。
    pub fn rates_file(&self) -> PathBuf {
        self.var_cache().join(crate::usage::rates::FILE)
    }

    /// 核心生成的配置 Schema、参考文件：`var/cache/config/`（施工 8-1）。
    pub fn config_reference(&self) -> PathBuf {
        self.var_cache().join("config")
    }

    /// 拉到的供应商模型列表、目录答的记录：`var/cache/models/`（施工 8-7；和 [`DataRoot::model_state`] 分开）。
    pub fn model_cache(&self) -> PathBuf {
        self.var_cache().join("models")
    }

    /// 官方软件源的索引缓存：`var/cache/packages/sync/`（施工 G-3）。
    pub fn official_feed(&self) -> PathBuf {
        self.var_cache().join("packages").join("sync")
    }

    /// 配置里加的别的软件源的索引缓存：`var/cache/packages/sources/`，下面一个源一个目录（施工 U23 下下）。
    pub fn other_feeds(&self) -> PathBuf {
        self.var_cache().join("packages").join("sources")
    }

    /// 下载的包文件：`var/cache/packages/downloads/`（施工 G-4）。
    pub fn package_downloads(&self) -> PathBuf {
        self.var_cache().join("packages").join("downloads")
    }

    /// MCP 服务 `service` 的工具目录缓存：`var/cache/mcp/<服务>.json`（施工 X-3 上）。
    pub fn mcp_catalog(&self, service: &str) -> PathBuf {
        self.var_cache().join("mcp").join(format!("{service}.json"))
    }

    /// 包 `package` 登记的工具原文缓存：`var/cache/provide/<包>.json`。
    pub fn provide_cache(&self, package: &str) -> PathBuf {
        self.var_cache()
            .join("provide")
            .join(format!("{package}.json"))
    }

    /// 脚本的缓存（`MIYU_SCRIPT_CACHE_DIR` 的根，施工 Y-11 上）：`var/cache/scripts/`，下面是 `[@<场所>/]<来处>/<工具>/`；30 天没碰的清掉
    /// （[`crate::cache`] 只扫这一处）。
    pub fn script_caches(&self) -> PathBuf {
        self.var_cache().join("scripts")
    }

    /// 人格图片的版本、主色缓存所在的目录：`var/cache/personas/`，文件 `pictures.json`（施工 P-7）。
    pub fn persona_picture_cache(&self) -> PathBuf {
        self.var_cache().join("personas")
    }

    /// mermaid 的字体缓存：`var/cache/mermaid/`。
    pub fn mermaid_cache(&self) -> PathBuf {
        self.var_cache().join("mermaid")
    }

    /// 探到的沙盒能力记在哪个目录：`var/cache/`，文件 `sandbox-probe.json`（施工 5-4 下）。
    pub fn sandbox_probe_dir(&self) -> PathBuf {
        self.var_cache()
    }

    /// 性能测试工具自己的缓存：`var/cache/perf/`。
    pub fn perf_cache(&self) -> PathBuf {
        self.var_cache().join("perf")
    }

    // ---- 日志（设计 55：`var/log/`） ----

    /// 运行日志的目录：`var/log/`（核心的 `core.log`、`memory.jsonl`）。
    pub fn logs(&self) -> PathBuf {
        self.var_log()
    }

    /// 包 `id` 自己写日志的目录：`var/log/<编号>/`（施工 FH-2下，设计 55 第四节的 `MIYU_LOG_DIR`）。不建：用的一方建。核心
    /// 记的扩展 stderr（[`DataRoot::extension_stderr`]）也落在这个目录里，叫 `stderr`、`stderr.old`，包自己不用这两个名字。
    pub fn package_logs(&self, id: &str) -> PathBuf {
        self.logs().join(id)
    }

    /// 扩展（包 `id` 的那一份）的标准错误：`var/log/<编号>/stderr`（施工 F-10；太大的挪成旁边的 `stderr.old`）。
    pub fn extension_stderr(&self, id: &str) -> PathBuf {
        self.package_logs(id).join("stderr")
    }

    /// MCP 服务 `service` 的标准错误：`var/log/mcp/<服务名>.stderr`（施工 X-4）。
    pub fn mcp_stderr(&self, service: &str) -> PathBuf {
        self.logs().join("mcp").join(format!("{service}.stderr"))
    }

    // ---- 暂存（设计 55：`tmp/`） ----

    /// 装 `.miyu` 时解包的暂存：`tmp/unpack/`（施工 G-1；核心起来清空）。
    pub fn unpack_staging(&self) -> PathBuf {
        self.tmp().join("unpack")
    }

    // ---- 包在各层各占的一个 `<编号>/`（设计 55 第三节第 4 条） ----

    /// 包 `id` 自己的配置：`etc/<编号>/`（`MIYU_CONFIG_DIR`）。不建。
    pub fn package_config(&self, id: &str) -> PathBuf {
        self.etc().join(id)
    }

    /// 包 `id` 能删的缓存：`var/cache/<编号>/`（`MIYU_CACHE_DIR`）。不建。
    pub fn package_cache(&self, id: &str) -> PathBuf {
        self.var_cache().join(id)
    }

    /// 包 `id` 短命的暂存：`tmp/<编号>/`（`MIYU_TMP_DIR`；核心拉起它的扩展之前清）。不建。
    pub fn package_tmp(&self, id: &str) -> PathBuf {
        self.tmp().join(id)
    }

    /// 包 `id` 的运行时目录：`run/<编号>/`（`MIYU_RUNTIME_DIR`）。不建。
    pub fn package_runtime(&self, id: &str) -> PathBuf {
        self.run().join(id)
    }

    /// 卸包 `id` 时整个删的那几个目录（设计 55 第三节第 4 条）：`etc/`、`var/lib/`、`var/cache/`、`var/log/`、`tmp/`、`run/` 下
    /// 它的 `<编号>/`；其中 `etc/<编号>/` 照 pacman 默认留下，`purge` 的才删（施工 PK-5，装卸那边挑）。包文件夹 `opt/<编号>/`、
    /// 本地库那一份由装卸那边删；它带的脚本、MCP 服务、知识库的几处照名字各自清。
    pub fn package_places(&self, id: &str) -> [PathBuf; 6] {
        [
            self.package_config(id),
            self.package_state(id),
            self.package_cache(id),
            self.package_logs(id),
            self.package_tmp(id),
            self.package_runtime(id),
        ]
    }
}

#[cfg(test)]
mod tests;
