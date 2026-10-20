//! 给界面用的门面（设计 `32-仓库拆分.md` 第二节，施工 S-1）：终端、网页、接入QQ 这些头连核心要的几样都从这里拿，不再直接
//! 依赖核心里别的 crate。代码大多留在原来的 crate，这里 `pub use` 转出去；门面上有的就是界面能用的，改了、少了照改协议的
//! 规矩告诉两个头（`tests.rs` 把每一样用一遍钉住）。
//!
//! - [`connect`]：经本机传输连核心、没在跑时拉起来（`miyu-ipc`）。
//! - [`open`]：照终端的样子连核心（出示本机令牌）、一问一答，带一次性码开浏览器（从 `miyu-webserve` 挪来）。
//! - [`places`]：数据根、资源目录、环境、语言、缓存目录在哪（`miyu-store`）。
//! - [`texts`]：给人看的字（`miyu-store` 的 `human`）。
//! - [`protocol`]：协议里的类型：编号、事件、模板（`miyu-kernel`）。
//! - [`manifest`]：读自己的软件包清单（`miyu-config`）。
//! - [`log`]：运行日志（`miyu-log`）。
//! - [`seeds`]：从一张图挑主题色的候选，跟随壁纸用（`miyu-seeds`，施工 P-6 补）：和核心算头像的同一份。
//! - [`heap`]：进程的堆：glibc 上限 arena、定大块门槛，空着的还给系统，自己占了多少（`miyu-heap`，施工 S-1 补）。
//!
//! 接入QQ 拆出核心仓库以前先只依赖门面（设计 32 第九节第 1 条，施工 S-6 上），多这几组；表情包这类第三方扩展要的也是它们：
//!
//! - [`ids`]、[`time`]：编号、时刻和时区偏移（`miyu-kernel`）。
//! - [`events`]：会话日志里的事件（`miyu-kernel`）。
//! - `tooltext`：工具的字怎么读（`miyu-tool`；feature `tool`，默认不开，所以这里不写成链接）。
//! - [`settings`]：配置的类型、读法和问题（`miyu-config`）。
//! - [`widgets`]：界面块内容的查验和挑语言（`miyu-config`，施工 X-28）。
//! - `http`：HTTP 客户端（`miyu-http`；feature `http`，同上）。

pub mod open;
#[cfg(test)]
mod tests;

use std::process::Command;
use std::sync::Arc;

/// 拉起核心的命令：主程序 `miyu` 加 `core`。
pub type CoreCommand = Arc<dyn Fn() -> Command + Send + Sync>;

/// 照钉住的那份核心检出的根目录（仓库根）：界面的测试照它读 `resources/`、`docs/designs/samples/`（设计 32 第四节第 3 条）。
/// git 依赖在 cargo 的检出里是整个仓库，资源和样本跟着钉住的提交走。只在 `testkit` 打开时有。
#[cfg(feature = "testkit")]
pub const WORKSPACE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// 经本机传输连核心、没在跑时拉起来。
pub mod connect {
    pub use miyu_ipc::{
        ConnectError, Connection, Ready, StartError, connect, connect_or_start,
        connect_or_start_bare, spawn_detached,
    };
}

/// 数据根、资源目录、环境、语言、缓存目录在哪；生成的文件怎么写、配置文件怎么读、两层的软件包清单（施工 S-6 上）。
pub mod places {
    pub use miyu_store::config_file;
    pub use miyu_store::env::{Env, Platform, locale};
    pub use miyu_store::generated::write;
    pub use miyu_store::packages::{Issue, Packages};
    pub use miyu_store::resources::ResourceRoot;
    pub use miyu_store::root::{DataRoot, cache_root};
}

/// 给人看的字：照语言读出来、画工具的显示名和说法。
pub mod texts {
    pub use miyu_store::human::{Face, Human, HumanError, clean};
}

/// 编号（施工 S-6 上）：平台上的人、场所、日志里的序号、内容的哈希。
pub mod ids {
    pub use miyu_kernel::id::{ContentHash, ExternalId, Seq, VenueId};
}

/// 时刻和时区偏移（施工 S-6 上）。
pub mod time {
    pub use miyu_kernel::time::{Timestamp, UtcOffset};
}

/// 会话日志里的事件（施工 S-6 上）：事件、正文、谁做的、内容块，写法不对时的错误。
pub mod events {
    pub use miyu_kernel::FormatError;
    pub use miyu_kernel::block::Block;
    pub use miyu_kernel::event::{Body, Event};
    pub use miyu_kernel::origin::By;
}

/// 工具的字怎么读（施工 S-6 上；feature `tool`，默认不开）：说明和参数格式、输出里给她看的几句（`load`），读好的说明、访问类别。
#[cfg(feature = "tool")]
pub mod tooltext {
    pub use miyu_kernel::tool::Access;
    pub use miyu_tool::Spec;
    pub use miyu_tool::load;
}

/// 配置的类型、读法和问题（施工 S-6 上）：值的种类、密钥、照种类读一个值、问题的原因码和位置。
pub mod settings {
    pub use miyu_config::Kind;
    pub use miyu_config::secret::Secret;
    /// 照种类读一个 TOML 的值、把解析器的原话说短。
    pub mod parse {
        pub use miyu_config::parse::{read, why};
    }
    /// 问题的原因码、轻重、位置，原文怎么印、最像的名字、说成话。
    pub mod problem {
        pub use miyu_config::problem::{At, Code, Severity, got, nearest, tell};
    }
}

/// HTTP 客户端（施工 S-6 上；feature `http`，默认不开）：照核心的规矩下载（只 http、https，超时、代理照它），认本机回环的地址。
#[cfg(feature = "http")]
pub mod http {
    pub use miyu_http::{Get, Got, Proxy, fetcher, get, is_loopback_url};
}

/// 协议里的类型。
pub mod protocol {
    pub use miyu_kernel::event::Said;
    pub use miyu_kernel::id::SessionId;
    pub use miyu_kernel::template::Template;
}

/// 界面块的内容（施工 X-28，接入QQ 的插件往它的页面加卡片、浮窗、整页要的）：查写法（`check`、`check_title`），照连接的
/// 语言挑字（`localize`、`pick`）。和核心收 `widget.set` 时查的是同一份（`docs/blueprint/widgets.md`「内容」）。
pub mod widgets {
    pub use miyu_config::package::WidgetPlace;
    pub use miyu_config::widget::{Bad, Problem, check, check_title, localize, pick};
}

/// 读自己的软件包清单、清单里的值。
pub mod manifest {
    pub use miyu_config::Value;
    pub use miyu_config::package::read;
}

/// 运行日志。
pub mod log {
    pub use miyu_log::{LevelFilter, install};
}

/// 从一张图挑主题色的候选（施工 P-6 补）：头算壁纸的候选用它，和核心算人格头像的候选（`persona.list` 的 `seeds`）是同一份
/// 解码、缩图、量化、打分，挑出来的一样。
pub mod seeds {
    pub use miyu_seeds::{MAX, Seed, from_image, from_rgba};
}

/// 进程的堆（施工 S-1 补）：和核心用的同一份。头起来时、还没起别的线程时调一次 [`heap::tune`]；不调的话 glibc 放掉一大块以后
/// 自己抬高大块门槛，之后的大块落进堆里碎掉、还不回去（终端画 23 张图量过：53 MiB，调了 23 MiB）。别的平台什么都不做。
pub mod heap {
    pub use miyu_heap::usage::{Usage, resident, usage};
    pub use miyu_heap::{Stats, stats, trim, tune};
}
