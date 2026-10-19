//! 端点的配置项（`docs/blueprint/config.md`「M8 的配置项」）：界面语言 `ui.language`（施工 8-1），新会话的权限级别
//! `permission.level`（施工 8-32），新会话开局只读
//! `permission.start_read_only`（施工 8-2），默认人格 `persona.default`（施工 P-1 上），终端会话 `terminal.*`（施工 8-33），软件源 `packages.source`、`packages.mirrors`
//! （施工 G-3 上）。
//!
//! 核心起来时照 `ui.language` 的最终值挑生成的文件用哪种语言；握手时照它和头报的系统语言算这个连接的语言（第二条
//! 第 8 条）。造会话时照 `permission.start_read_only` 的最终值（带上信任着的项目配置）定开局是不是只读（第二条第 9 条）。

use miyu_config::Address;

miyu_config::settings! {
    /// 界面的配置。
    pub struct UiSettings in "ui" {
        /// 界面语言：`auto` 跟着系统，或者定成 `zh`、`en`、`ja`。
        language: String = "auto" {
            kind: option ["auto", "zh", "en", "ja"],
            layers: [System, Personal],
            applies: now,
            ui: { page: "general", group: "display", common: true, control: select },
        },
        /// 打开一个头（终端界面、网页）时开哪个会话：`new` 开一个新的，`recent` 接着最近的那一个（施工 8-28，2026-10-07 项目主人定：
        /// 同一个人在同一台机器上，每个头的体验一样，几个头都有的行为是一个共用的项；原来是 `tui.startup`）。头自己经
        /// `config.get` 读、起来时读一次，核心不管它。
        startup: String = "new" {
            kind: option ["new", "recent"],
            layers: [System, Personal],
            applies: head_start,
            ui: { page: "general", group: "display", control: select },
        },
        /// 直接敲 `miyu`、`miyu config` 时打开哪个界面：软件包的编号（施工 9-3，`packages.md`「入口」）。主程序每次敲的时候经
        /// `config.get` 读，照清单找这个包的程序。
        head: String = "tui" {
            kind: name,
            layers: [System, Personal],
            applies: head_start,
            ui: { page: "general", group: "display", control: text },
        },
        /// 第一次引导走过了（施工 8-11 四补，2026-10-08 终端、网页两个头要的）：账号级，只在个人设置里，每个账号各走一次；
        /// 头走完引导写 `true`，以后不再进，缺什么去配置页补。设置页不画。核心不读它。
        welcomed: bool = false {
            kind: bool,
            layers: [Personal],
            applies: now,
            ui: { page: "general", group: "display", control: toggle, hidden: true },
        },
    }
}

miyu_config::settings! {
    /// 人格的配置（施工 P-1 上，`docs/blueprint/personas.md`）。
    pub struct PersonaSettings in "persona" {
        /// 新会话默认用哪个人格：人格目录的编号。开会话时指定的、通讯平台的桥照场所规则交来的优先。出厂不设：没设的新会话
        /// 无人格（施工 P-4 上）；指着没有的人格的当没设。
        default: Option<String> = none {
            kind: name,
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "general", group: "persona", control: text },
        },
    }
}

miyu_config::settings! {
    /// 预设的配置（施工 P-2 上，`docs/blueprint/presets.md`）。
    pub struct PresetSettings in "preset" {
        /// 新会话默认用哪个预设：预设文件的编号。开会话时指定的、通讯平台的桥照场所规则交来的优先。指着没有的不悄悄换
        /// （Y12）。
        default: String = "full" {
            kind: name,
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "general", group: "preset", control: text },
        },
    }
}

miyu_config::settings! {
    /// 终端会话（施工 8-33，设计 `22-命令行.md` 第七节，2026-10-20 项目主人定）：在 shell 里说的话进的那个一直续下去的会话。
    /// `session.terminal` 照它开新的、换新的。
    pub struct TerminalSettings in "terminal" {
        /// 新开的终端会话用哪个人格：人格目录的编号。没设的跟随 `persona.default`。只管新开的，在用的不换。
        persona: Option<String> = none {
            kind: name,
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "general", group: "terminal", control: text },
        },
        /// 新开的终端会话用哪个预设：预设文件的编号。没设的跟随 `preset.default`。
        preset: Option<String> = none {
            kind: name,
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "general", group: "terminal", control: text },
        },
        /// 上一次说话以后闲置超过多少分钟，下一句开一个新的终端会话；0 是不换。默认 12 小时（2026-10-20 项目主人定）。
        idle_new_after: i64 = 720 {
            kind: int [0, 525600],
            layers: [System, Personal],
            applies: now,
            ui: { page: "general", group: "terminal", control: number },
        },
    }
}

miyu_config::settings! {
    /// 权限的配置（施工 8-2）。
    pub struct PermissionSettings in "permission" {
        /// 新会话一开局就是只读：她只能查、写计划。项目配置只能把它打开。
        start_read_only: bool = false {
            kind: bool,
            layers: [System, Personal, Project],
            tighten: true_only,
            applies: new_session,
            ui: { page: "general", group: "sessions", control: toggle },
        },
        /// 新会话开局的权限级别（施工 8-32）：`workspace` 是工作区（执行命令在沙盒里跑），`full` 是完全放开。头的第一次
        /// 引导里「沙盒」那一步写它，开是 `workspace`（2026-10-13 项目主人定：默认开）。只管本机的会话：场所会话照旧从工作区开局。
        level: String = "workspace" {
            kind: option ["workspace", "full"],
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "general", group: "sessions", control: select },
        },
    }
}

miyu_config::settings! {
    /// 一条斜杠命令的配置（施工 O-6 六补，`slash.md`「谁能用」，接入QQ 的会话 2026-10-17 要的）：`<id>` 是命令名（写法同「路径里的名字」，命令名都合）。
    pub struct CommandSettings in "commands.<id>" {
        /// 谁能用：`admin` 本人、对应表里认得的、场所里管理的人；`anyone` 谁都行；`off` 谁都不行。没写的照命令的出厂值（扩展
        /// 清单 `[[slash]]` 的 `access`），也没有的是 `admin`。设置页不画：带名字的这一段两个头还没有画法。
        access: Option<String> = none {
            kind: option ["admin", "anyone", "off"],
            layers: [System],
            applies: now,
            ui: { page: "general", group: "commands", control: select, hidden: true },
        },
    }
}

/// 跟着系统。
const AUTO: &str = "auto";

impl UiSettings {
    /// 给人看的字用哪种语言：`zh`、`en`、`ja` 之一（`config.md` 第二条第 8 条）。`language` 定了的就是它；是 `auto` 的
    /// 照系统的语言 `locale`：`zh` 开头的是 `zh`，`ja` 开头的是 `ja`，别的、没有的是 `en`。
    pub fn language_for(&self, locale: Option<&str>) -> &str {
        if self.language != AUTO {
            return &self.language;
        }
        match locale {
            Some(locale) if locale.starts_with("zh") => "zh",
            Some(locale) if locale.starts_with("ja") => "ja",
            _ => "en",
        }
    }
}

miyu_config::settings! {
    /// 软件源（施工 G-3 上，设计 `37-软件源.md` 第五节）：从哪拿索引、下包。整台机器一份，只在系统配置里；当场生效。
    pub struct SourceSettings in "packages" {
        /// 软件源的地址：放 `index.json`、`index.json.sig` 的那一层目录。索引照编进来的官方公钥验，换成别的地址也得是官方签的。
        source: Address = "https://miyu-agent.github.io/packages/" {
            kind: url,
            layers: [System],
            applies: now,
            ui: { page: "packages", group: "source", control: text },
        },
        /// 镜像：照先后试，都不成再试上面那个地址。一个镜像同样放 `index.json`、`index.json.sig`，包文件在 `files/` 下面。
        mirrors: Vec<Address> = [] {
            kind: urls,
            layers: [System],
            applies: now,
            ui: { page: "packages", group: "source", control: list },
        },
        /// 升级时跳过的包（施工 G-5，照 pacman 的 `IgnorePkg`）：编号的列表。
        hold: Option<Vec<String>> = none {
            kind: texts [64],
            layers: [System],
            applies: now,
            ui: { page: "packages", group: "source", control: list },
        },
    }
}

#[cfg(test)]
mod tests;

/// 终端管理员的平台账号（施工 O-3；以前叫主人对应表，`docs/blueprint/venues.md`）：`external.bindings.<external>`，键是通讯平台上的身份，值是本机账号。
/// `settings!` 只认「段加字段名」，最后一段是占位的这一项手写。只能写在系统配置，当场生效：下一句就照新的认。
pub const EXTERNAL_BINDINGS: &[miyu_config::Item] = &[miyu_config::Item {
    key: "external.bindings.<external>",
    kind: miyu_config::Kind::Name,
    default: None,
    layers: &[miyu_config::Layer::System],
    tighten: None,
    env: None,
    applies: miyu_config::Applies::Now,
    ui: miyu_config::Ui {
        page: "advanced",
        group: "external",
        package: None,
        common: false,
        control: miyu_config::Control::Text,
        hidden: false,
    },
}];
