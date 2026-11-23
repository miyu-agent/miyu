//! 逛一圈（施工 LW-14，`internal/blueprint/perf.md`「排函数」）：配着真的终端界面录的第三、四段，说完话、做完工具调用
//! 以后，照 [`steps`] 的先后把常用的页面和命令用一遍。这些路剧本以前没有走到，函数散在清单以外，真用时打开一次设置页
//! 核心多 1.8 MB、终端界面多 1.3 MB 代码页（2026-11-22 实测）。
//!
//! 每一步发完按键，等屏幕上（这一步发键以后新画的、去掉转义序列和空格）出现这一步等的字再往下；等不到报错停下，报的是
//! 哪一步、等的什么字、屏幕上最后是什么。等的字照终端界面钉的那一份抄（2026-11-22，miyu-tui `ff3834fa`）：终端界面改了
//! 这些页面的字，换 `heads.toml` 的钉时打包会在这里停下，改这张表。
//!
//! 关页面的一步等的是 [`BACK`]：回到对话的时候，终端界面把页面盖住的那几行重画，最后一轮底下那一行写着模型（`perf/perf-model`），
//! 页面上不这样写；这一行没有重画的时候，重画出来的工具调用那几行的 `Checking.` 也算。终端界面只重画变了的字格，所以等的字挑页面上独有、不会恰好和底下同一格同一个字的。

use std::time::{Duration, Instant};

use super::head::Turns;
use super::terminal::Terminal;
use crate::fake::Fake;
use crate::fake::script::PREAMBLE;
use crate::measure::tools::calls::Workspace;

/// 逛一圈每一步等屏幕上的字最多等多久。字对不上（终端界面改了页面的字）时不用白等十分钟（`order/head.rs` 的
/// `PATIENCE` 是 600 秒）；模拟器里最慢的一步实测见施工单 `LW-14-录清单的剧本补上常用操作和抽记忆.md`「量到的」，这里留了
/// 三倍以上的余量。等假模型的请求、等一轮结束仍然照 `PATIENCE`。
pub(super) const STEP_PATIENCE: Duration = Duration::from_secs(120);

/// 关页面的几步等的「回到对话」：最后一轮底下那一行的模型。页面盖住的那几行重画时，这一行有时和打开以前落在同一格、
/// 终端界面不重画它（2026-11-22 第四段退格关斜杠菜单时实测），所以重画出来的工具调用那几行（[`BACK_TOO`]）也算。
pub const BACK: &str = "perf/perf-model";

/// 回到对话时也算数的另一种字：工具调用那几行开头的 [`PREAMBLE`]。
const BACK_TOO: &str = PREAMBLE;

/// 设置页的分类页底下那一行（`j/k 移动 Enter 进入 q/Esc 返回`）：进了分类再退回来时重画，分类里面的页底下没有这一句。
const MENU: &str = "Enter 进入";

/// 进「通用」以后等的字：「新会话」一组里的一项，分类页上的说明里没有。
const GENERAL: &str = "新会话默认人格";

/// 设置页里「外观」后面的几个分类，照分类页上的先后（终端界面 `ff3834fa`，量尺的沙箱里实际列出来的九个分类里除去
/// 「通用」「外观」，2026-11-22 看屏幕定的）：名字、分类页上它底下那一行的说明（往下选到它时这一行换了样子、重画）、
/// 进去以后等的字（这一页独有、分类页上没有的；页签那一行用「·」隔开，分类页上的说明用「、」）。「预设」只在核心给了
/// 预设时列，「软件包」一直列；核心以后多了分类、换了先后，这张表跟着改。进「软件包」时核心去取软件包的目录（要连网），
/// 等的是页签那一行，取没取到都画。
const CATEGORIES: [(&str, &str, &str); 7] = [
    (
        "供应商和模型",
        "供应商、默认模型、模型池",
        "供应商 · 默认模型 · 模型池",
    ),
    ("用户", "个人资料", "回复语言"),
    ("隐私与安全", "受保护的命令、权限", "破坏性 git 操作"),
    ("人格", "决定 AI 的人格和记忆", "先用最简单的办法"),
    ("预设", "决定开关哪些功能", "极简开发"),
    (
        "软件包",
        "下载软件、已安装、软件后台",
        "下载软件 · 已安装 · 软件后台",
    ),
    ("高级", "压缩、核心、运行日志、技能", "压缩触发线"),
];

/// `/theme` 里往下选到的那一项（沙箱里只列「自动」和它）：选中的那一行重画，终端界面照它预览配色。
const THEME_NEXT: &str = "终端配色";

/// macOS 上（施工 MO-2）`/theme` 在「自动」下面先列「跟随壁纸」：终端界面在那里找得到桌面的壁纸，不看沙箱的家目录
/// （2026-11-23 在一台 M1 MacBook 上看到的）。等 [`THEME_NEXT`] 的时候等到它也算（[`tour`] 里等字的那一处加上，步骤表
/// 不变）；Linux 的沙箱里不列它，那边照旧只等得到 [`THEME_NEXT`]。
const THEME_WALLPAPER: &str = "跟随壁纸";

/// 按键。
const ENTER: &[u8] = b"\r";
const ESC: &[u8] = b"\x1b";
const DOWN: &[u8] = b"\x1b[B";
const PAGE_UP: &[u8] = b"\x1b[5~";
const PAGE_DOWN: &[u8] = b"\x1b[6~";
const BACKSPACE: &[u8] = b"\x7f";

/// 逛一圈的一步：`name` 报错时说是哪一步。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub name: String,
    pub act: Act,
}

/// 一步做什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// 按 `keys`，等屏幕上出现 `until`；`or` 给了的，出现它也算（哪几行重画说不准的那几步）。
    Keys {
        keys: &'static [u8],
        until: &'static str,
        or: Option<&'static str>,
    },
    /// 说一句，等假模型收到这一轮的主请求、屏幕上画出回答、这一轮结束（`/new` 以后的新会话里用）。
    Say,
    /// 说一句，假模型回一次 `tool` 的调用（参数是 `args`，里面的 `{dir}` 换成工作区的目录），屏幕上画出 `shown` 以后，
    /// `answer` 的按一下回车（提问选第一项、确认框选「允许本次」），等这一轮结束。
    Call {
        tool: &'static str,
        args: &'static str,
        shown: &'static str,
        answer: bool,
    },
    /// 说一句，假模型对这一轮的头一个请求回 500，等核心重试、屏幕上画出重试的那一行、这一轮结束。
    Retry,
    /// 说一句，假模型慢慢地一块块回；屏幕上画出回答的头几个字以后按两下 Esc 停下，等屏幕上画出已中断、这一轮结束。
    Stop,
}

/// 按 `keys` 等 `until` 的一步。
fn key(name: &str, keys: &'static [u8], until: &'static str) -> Step {
    let or = (until == BACK).then_some(BACK_TOO);
    Step {
        name: name.to_string(),
        act: Act::Keys { keys, until, or },
    }
}

/// 打一条斜杠命令 `command`（等它出现在输入框里）、回车，等 `until`。
fn command(command: &'static str, until: &'static str) -> [Step; 2] {
    [
        key(command, command.as_bytes(), command),
        key(&format!("{command} 回车"), ENTER, until),
    ]
}

/// 打一条斜杠命令、回车，等它的页面画出 `page`；再按 Esc，等回到对话（[`BACK`]）。
fn page(name: &'static str, page: &'static str) -> [Step; 3] {
    let [typed, entered] = command(name, page);
    [
        typed,
        entered,
        key(&format!("{name} 按 Esc 关掉"), ESC, BACK),
    ]
}

/// 说话的一步（[`Act::Say`]、[`Act::Call`]、[`Act::Retry`]、[`Act::Stop`]）。
fn talk(name: &str, act: Act) -> Step {
    Step {
        name: name.to_string(),
        act,
    }
}

/// 逛一圈的先后：一行一步，名字、按键、等的字。
pub fn steps() -> Vec<Step> {
    let mut steps = vec![
        key("往上翻一页", PAGE_UP, "跳转到底部"),
        // 翻回来时最后一轮底下那一行常常和翻之前落在同一格，终端界面不重画它；挪了位置的工具调用那几行每次都重画。
        key("往下翻回来", PAGE_DOWN, PREAMBLE),
        key("打 / 开斜杠菜单", b"/", "命令"),
        key("退格关掉斜杠菜单", BACKSPACE, BACK),
    ];
    for (name, until) in [
        ("/help", "设定长期目标"),
        ("/status", "权限模式"),
        ("/context", "系统提示词"),
        ("/usage", "单日最高"),
        ("/tool-list", "ask_user"),
    ] {
        steps.extend(page(name, until));
    }
    // 设置页：先进「通用」再退回分类，往下选到「外观」进去、打开「配色」的选单，再一层层退回分类；然后往下把剩下的分类
    // 一个个进一遍、退回来，最后关掉设置页。
    steps.extend(command("/config", "隐私与安全"));
    steps.extend([
        key("/config 进通用", ENTER, GENERAL),
        key("/config 从通用退回分类", ESC, MENU),
        key("/config 往下选到外观", DOWN, "主题、配色方案"),
        key("/config 进外观", ENTER, "首页形象"),
        key("/config 往下选到配色", DOWN, "配色"),
        key("/config 打开配色的选单", ENTER, "Esc 取消"),
        key("/config 按 Esc 关掉选单", ESC, "Esc 返回"),
        key("/config 从外观退回分类", ESC, MENU),
    ]);
    for (category, described, inside) in CATEGORIES {
        steps.extend([
            key(&format!("/config 往下选到{category}"), DOWN, described),
            key(&format!("/config 进{category}"), ENTER, inside),
            key(&format!("/config 从{category}退回分类"), ESC, MENU),
        ]);
    }
    steps.push(key("/config 按 Esc 关掉设置页", ESC, BACK));
    for (name, until) in [
        ("/model", "perftls"),
        ("/persona", "不使用人格"),
        ("/preset", "开放全部工具"),
    ] {
        steps.extend(page(name, until));
    }
    // 配色：往下选一项，终端界面照它预览；按 Esc 关掉，回到原来的配色。
    steps.extend(command("/theme", "终端配色"));
    steps.extend([
        key("/theme 往下选一项预览", DOWN, THEME_NEXT),
        key("/theme 按 Esc 关掉", ESC, BACK),
    ]);
    for (name, until) in [
        ("/files", "切换文件"),
        ("/jump", "跳过去"),
        ("/sessions", "置顶"),
    ] {
        steps.extend(page(name, until));
    }
    steps.extend(command("/compact", "没有可压缩的内容"));
    steps.extend(command("/new", "开放权限"));
    // 新会话里说一句，再做几种平常会碰到、工具一步没有的（施工 LW-14 先量过，核心或终端界面多 256 kB 以上的才加）。
    let call = |tool, args, shown, answer| Act::Call {
        tool,
        args,
        shown,
        answer,
    };
    steps.extend([
        talk("/new 以后说一句", Act::Say),
        talk(
            "记待办",
            call(
                "todowrite",
                r#"{"todos": [{"content": "measure", "status": "in_progress"}, {"content": "report", "status": "pending"}]}"#,
                "待办",
                false,
            ),
        ),
        talk(
            "提问，选第一项",
            call(
                "ask_user",
                r#"{"questions": [{"question": "Which one?", "header": "Pick", "options": [{"label": "Alpha"}, {"label": "Beta"}]}]}"#,
                "Enter 提交",
                true,
            ),
        ),
        // 受保护的命令：完全放开下也问人。工作区是量尺自己 `git init` 的空仓库，没有提交，放行了也只是报错。仓库和工作
        // 目录都写死成工作区（`--git-dir`、`--work-tree`），不让 git 自己往上找：量尺的沙盒在源码树的 `target/` 下，工作区的
        // `.git` 万一不是一个能用的仓库，`git -C <工作区>` 会找到外面的源码树、把那里没提交的改动丢掉。
        talk(
            "受保护的命令，允许本次",
            call(
                "shell",
                r#"{"command": "git --git-dir='{dir}/.git' --work-tree='{dir}' reset --hard", "description": "reset"}"#,
                "允许本次",
                true,
            ),
        ),
        talk(
            "派子代理",
            call(
                "subagent",
                r#"{"description": "measure a child", "prompt": "Answer in one line."}"#,
                // 子代理一下就答完了，「派子代理」那一行一闪而过；收起来以后的这一行一直在。它跟着界面语言写（终端界面
                // 2026-11-23 起撤掉了「跟随系统时收起的那一行写英文」），中英两种都认（施工 FX-24）。只等开头那一截：
                // 终端界面只重画变了的字格，2026-11-23 在一台 M1 MacBook 上去掉转义序列以后屏幕上是「Spawned 1 agnt」，整句
                // 等不到（施工 MO-4）。
                "Spawned 1 ag|派了 1",
                false,
            ),
        ),
        talk("模型回 500 以后重试", Act::Retry),
        talk("说话途中按两下 Esc 停下", Act::Stop),
    ]);
    // 回到上一个会话：列表里第二个是它（新的在前），回去以后侧栏画出它改过的文件。
    steps.extend(command("/sessions", "置顶"));
    steps.extend([
        key("/sessions 往下选到上一个会话", DOWN, "❯"),
        key("/sessions 回车切过去", ENTER, "out.txt"),
    ]);
    // 撤回以后那一行写「已撤销」（`ff3834fa`）或「已撤回」（之后的终端界面），后半句「/restore 恢复」两种都有；这一行
    // 有时和输入框的提示落在同一格、不重画（2026-11-22 第四段实测），撤回的那一句回到输入框里（「…-tools: go.」）也算。
    let [typed, mut entered] = command("/undo", "/restore 恢复");
    if let Act::Keys { or, .. } = &mut entered.act {
        *or = Some("tools: go.");
    }
    steps.extend([typed, entered]);
    steps
}

/// 照 [`steps`] 逛一圈：说话的几步的记号是 `<tag>-tour-<第几步>`，工具调用的参数照工作区 `workspace`。
///
/// # Errors
///
/// 哪一步等不到：报是哪一步、等的什么字。
pub async fn tour(
    terminal: &mut Terminal,
    fake: &mut Fake,
    turns: &Turns<'_>,
    workspace: &Workspace,
    tag: &str,
) -> Result<(), String> {
    let mut slowest = (Duration::ZERO, String::new());
    for (at, Step { name, act }) in steps().into_iter().enumerate() {
        let what = format!("逛一圈「{name}」");
        let marker = format!("{tag}-tour-{at:02}");
        let mut talk = talk::Talk {
            terminal: &mut *terminal,
            fake: &mut *fake,
            turns,
            what: &what,
            marker: &marker,
        };
        match act {
            Act::Keys { keys, until, or } => {
                let began = Instant::now();
                let mark = talk.terminal.mark();
                talk.terminal.write(keys)?;
                let wallpaper = (until == THEME_NEXT).then_some(THEME_WALLPAPER);
                let needles: Vec<&str> =
                    [Some(until), or, wallpaper].into_iter().flatten().collect();
                talk.terminal
                    .wait_for_any(&needles, mark, STEP_PATIENCE, &what)
                    .await?;
                if began.elapsed() > slowest.0 {
                    slowest = (began.elapsed(), name);
                }
            }
            Act::Say => talk.say().await?,
            Act::Call {
                tool,
                args,
                shown,
                answer,
            } => {
                // 目录照 JSON 的字转义（去掉两头的引号）再换进去：路径里有引号、反斜杠的照样是合法的 JSON。
                let dir = serde_json::Value::from(workspace.dir.to_string_lossy()).to_string();
                let args = args.replace("{dir}", dir.trim_matches('"'));
                talk.call(tool, &args, shown, answer).await?;
            }
            Act::Retry => talk.retry().await?,
            Act::Stop => talk.stop().await?,
        }
    }
    // 只说按键的几步里最慢的那一步（发键到等到字）：定 [`STEP_PATIENCE`] 时照它留余量。
    eprintln!(
        "排函数：{tag} 逛一圈按键的几步里最慢的是「{}」，{:.1} 秒",
        slowest.1,
        slowest.0.as_secs_f64()
    );
    Ok(())
}

mod talk;

#[cfg(test)]
mod tests;
