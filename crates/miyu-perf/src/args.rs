//! 量尺的参数：几个路径必写，次数、大小有默认（`cargo xtask perf` 照仓库填路径）。

use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

/// 用法，参数不对时印出来。
pub const USAGE: &str = "用法：miyu-perf --miyu <release 的 miyu> --resources <resources/> --design <23-性能预算.md> --work <沙盒目录> --out <结果目录> [--runs 10] [--sessions 10] [--events 10000] [--tail 30] [--reloads 20] [--appends 2000] [--reply-bytes 3500] [--streams 10] [--pieces 300] [--idle-wait 200] [--gate 倍数]\n      miyu-perf --tools 20 --tree <拷进工作区的树> --miyu … --resources … --design … --work … --out … [--tools-log debug] [--tag 名]\n      miyu-perf --render <原始数据.json> --design <23-性能预算.md> --out <结果目录>\n      miyu-perf --order <清单文件> --emulator <qemu-x86_64> --tree <拷进工作区的树> --miyu … --resources … --work … [--head <终端界面程序> [--head-order <终端界面的清单文件>]]";

/// 量的时候必写的几个路径。
const PATHS: [&str; 5] = ["--miyu", "--resources", "--design", "--work", "--out"];

/// 照原始数据重出表时必写的。
const RENDER_PATHS: [&str; 2] = ["--design", "--out"];

/// 录排函数的清单时必写的（施工 LW-1）：不量、不出表，用不着预算和结果目录。
const ORDER_PATHS: [&str; 6] = [
    "--miyu",
    "--resources",
    "--work",
    "--tree",
    "--order",
    "--emulator",
];

/// 量什么、量多少次、放哪。
#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    /// release 构建的 `miyu`；`miyu-sandbox` 在它旁边。
    pub miyu: PathBuf,
    /// 资源目录：源码树的 `resources/`。
    pub resources: PathBuf,
    /// `internal/designs/23-性能预算.md`：预算照它第二节的表。
    pub design: PathBuf,
    /// 沙盒放哪：要在真的磁盘上，同步才算数。
    pub work: PathBuf,
    /// 结果写到哪个目录。
    pub out: PathBuf,
    /// 不量，照这一份原始数据、照现在的预算重出对照表（预算校准以后用，23 F1）。
    pub render: Option<PathBuf>,
    /// 冷启动量几次（先另外空跑一次）。
    pub runs: usize,
    /// 热启动、每多一个会话：几个小会话。
    pub sessions: usize,
    /// 大会话说到第几条事件。
    pub events: u64,
    /// 到了以后再说几轮：一次请求的投影照这几轮算分位。
    pub tail: usize,
    /// 重启核心再打开大会话几次。
    pub reloads: usize,
    /// 追加并同步：追加几条。
    pub appends: usize,
    /// 假模型每次回多少字节的正文。
    pub reply_bytes: usize,
    /// 流式一轮（施工 V-22）：在大会话上说几轮。
    pub streams: usize,
    /// 流式一轮：回答切成几块。
    pub pieces: usize,
    /// 不订阅大会话以后等多久再量内存：要比会话 actor 空闲退出的 180 秒长（`miyu-endpoint` 的 `sessions::idle::IDLE`）。
    pub idle_wait: Duration,
    /// 闸门（施工 V-3）：有预算的哪一项量到的超过预算的这么多倍，量完报错退出；没写的只出表。
    pub gate: Option<f64>,
    /// 工具一步（施工 V-29）：写了的只量这一场，每种调用量这么多次。
    pub tools: Option<usize>,
    /// 工具一步：拷进工作区的树，`grep`、`glob` 在它里面找（`cargo xtask perf` 填源码树的 `crates/`）。
    pub tree: Option<PathBuf>,
    /// 工具一步：核心的 `MIYU_LOG`，拆得更细时写 `debug`。
    pub tools_log: Option<String>,
    /// 工具一步：结果的文件名后面接的标签。
    pub tag: Option<String>,
    /// 排函数（施工 LW-1）：写了的不量，在模拟器里照剧本录一遍，把执行过的函数照头一回执行的先后写进这个文件。
    pub order: Option<PathBuf>,
    /// 排函数：QEMU 的用户态模拟器，例如 `qemu-x86_64`。要和 `--order` 一起写。
    pub emulator: Option<PathBuf>,
    /// 排函数（施工 LW-6）：真的终端界面程序。给了的在照协议问的两段以后，再配着它录第三段（核心在模拟器里，
    /// `order/head.rs`）。只在 `--order` 的跑法里认。
    pub head: Option<PathBuf>,
    /// 排函数：终端界面自己的清单写到哪（施工 LW-6）。给了的再录第四段（终端界面在模拟器里）；要和 [`Args::head`] 一起写。
    /// 只写 `--head` 的不录第四段：终端界面钉的提交还不认清单时，`cargo xtask dist` 只排核心。
    pub head_order: Option<PathBuf>,
}

/// 读命令行参数（不含程序名）。
///
/// # Errors
///
/// 缺了必写的、不认识的、数写坏了：交回哪里不对，后面跟着用法。
pub fn parse(words: &[String]) -> Result<Args, String> {
    let mut args = Args {
        miyu: PathBuf::new(),
        resources: PathBuf::new(),
        design: PathBuf::new(),
        work: PathBuf::new(),
        out: PathBuf::new(),
        render: None,
        runs: 10,
        sessions: 10,
        events: 10_000,
        tail: 30,
        reloads: 20,
        appends: 2000,
        reply_bytes: 3500,
        streams: 10,
        pieces: 300,
        idle_wait: Duration::from_secs(200),
        gate: None,
        tools: None,
        tree: None,
        tools_log: None,
        tag: None,
        order: None,
        emulator: None,
        head: None,
        head_order: None,
    };
    let mut given = Vec::new();
    let mut words = words.iter();
    while let Some(flag) = words.next() {
        let value = words
            .next()
            .ok_or_else(|| format!("{flag} 后面要跟一个值\n{USAGE}"))?;
        match flag.as_str() {
            "--miyu" => args.miyu = value.into(),
            "--resources" => args.resources = value.into(),
            "--design" => args.design = value.into(),
            "--work" => args.work = value.into(),
            "--out" => args.out = value.into(),
            "--render" => args.render = Some(value.into()),
            "--runs" => args.runs = number(flag, value)?,
            "--sessions" => args.sessions = number(flag, value)?,
            "--events" => args.events = number(flag, value)?,
            "--tail" => args.tail = number(flag, value)?,
            "--reloads" => args.reloads = number(flag, value)?,
            "--appends" => args.appends = number(flag, value)?,
            "--reply-bytes" => args.reply_bytes = number(flag, value)?,
            "--streams" => args.streams = number(flag, value)?,
            "--pieces" => args.pieces = number(flag, value)?,
            "--idle-wait" => args.idle_wait = Duration::from_secs(number(flag, value)?),
            "--gate" => args.gate = Some(factor(flag, value)?),
            "--tools" => args.tools = Some(number(flag, value)?),
            "--tree" => args.tree = Some(value.into()),
            "--tools-log" => args.tools_log = Some(value.clone()),
            "--tag" => args.tag = Some(value.clone()),
            "--order" => args.order = Some(value.into()),
            "--emulator" => args.emulator = Some(value.into()),
            "--head" => args.head = Some(value.into()),
            "--head-order" => args.head_order = Some(value.into()),
            _ => return Err(format!("不认识 {flag}\n{USAGE}")),
        }
        given.push(flag.as_str());
    }
    if args.order.is_some() != args.emulator.is_some() {
        return Err(format!("--order 要和 --emulator 一起写\n{USAGE}"));
    }
    if args.head_order.is_some() && args.head.is_none() {
        return Err(format!("--head-order 要和 --head 一起写\n{USAGE}"));
    }
    if args.head.is_some() && args.order.is_none() {
        return Err(format!("--head 只在 --order 的跑法里认\n{USAGE}"));
    }
    let needed = if args.order.is_some() {
        &ORDER_PATHS[..]
    } else if args.render.is_some() {
        &RENDER_PATHS[..]
    } else {
        &PATHS[..]
    };
    if let Some(missing) = needed.iter().find(|path| !given.contains(path)) {
        return Err(format!("缺 {missing}\n{USAGE}"));
    }
    if args.tools.is_some() && args.tree.is_none() {
        return Err(format!("--tools 要和 --tree 一起写\n{USAGE}"));
    }
    Ok(args)
}

/// 一个正的倍数。
fn factor(flag: &str, value: &str) -> Result<f64, String> {
    match value.parse::<f64>() {
        Ok(factor) if factor.is_finite() && factor > 0.0 => Ok(factor),
        _ => Err(format!("{flag} 要一个正数，收到「{value}」\n{USAGE}")),
    }
}

/// 一个非负整数。
fn number<T: FromStr>(flag: &str, value: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{flag} 要一个非负整数，收到「{value}」\n{USAGE}"))
}

#[cfg(test)]
mod tests;
