//! 装包的进度怎么印（施工 IN-3，设计 `49-装包照pacman.md` 第二节第 4 条）：一行进度条照终端宽度排、百分比、窄终端；终端里刷新
//! 同一行，管道里只印行；没写阶段的老推送当下载。

use serde_json::{Value, json};

use super::{Progress, bar};
use crate::language::Language;

/// 10.1 MiB。
const EMBED: u64 = 10_590_618;

fn full(columns: usize) -> String {
    bar(("miyu-embed-0.3.0", 21), (EMBED, EMBED), columns)
}

#[test]
fn a_bar_fills_the_terminal_width_but_the_last_column() {
    let line = full(80);
    assert_eq!(
        line,
        format!(
            " miyu-embed-0.3.0{}10.1 MiB  [{}] 100%",
            " ".repeat(8),
            "#".repeat(37)
        )
    );
    assert_eq!(line.chars().count(), 79, "最后一格留着，免得折行");
    let wide = full(120);
    assert_eq!(wide.chars().count(), 119);
    assert_eq!(wide.matches('#').count(), 77, "终端越宽，条越长");
}

#[test]
fn the_bar_and_the_percent_follow_what_is_done() {
    let half = bar(("lib-1.0", 7), (500, 1000), 60);
    assert!(half.ends_with("]  50%"), "{half}");
    let inside = &half[half.find('[').expect("有条") + 1..half.find(']').expect("有条")];
    assert_eq!(inside.matches('#').count(), inside.len() / 2, "{half}");
    let third = bar(("lib-1.0", 7), (1, 3), 60);
    assert!(third.ends_with("  33%"), "{third}");
    let empty = bar(("lib-1.0", 7), (0, 0), 60);
    assert!(
        empty.ends_with("] 100%") && !empty.contains("#-") && !empty.contains("[-"),
        "{empty}"
    );
    let over = bar(("lib-1.0", 7), (5, 3), 60);
    assert!(over.ends_with("] 100%"), "多推了的不超过 100%：{over}");
}

#[test]
fn a_narrow_terminal_drops_the_bar_then_cuts_the_line() {
    assert_eq!(
        full(40),
        format!(" miyu-embed-0.3.0{}10.1 MiB 100%", " ".repeat(8)),
        "放不下十格的条就不画"
    );
    let cut = full(20);
    assert_eq!(cut, " miyu-embed-0.3.0  ", "再窄的从后面截掉");
    assert_eq!(full(0), "");
}

/// `lib` 下载（老的核心：不写阶段）、`app` 下载，再核对、安装两个。
fn pushes() -> Vec<Value> {
    let push =
        |params: Value| json!({"jsonrpc": "2.0", "method": "package.progress", "params": params});
    vec![
        push(json!({"package": "lib", "done": 0, "total": 1000})),
        push(json!({"package": "lib", "done": 1000, "total": 1000})),
        json!({"jsonrpc": "2.0", "method": "session.event", "params": {}}),
        push(
            json!({"package": "app", "stage": "download", "done": 0, "total": 2048, "index": 2, "count": 2}),
        ),
        push(
            json!({"package": "app", "stage": "download", "done": 2048, "total": 2048, "index": 2, "count": 2}),
        ),
        push(json!({"package": "lib", "stage": "verify", "index": 1, "count": 2})),
        push(json!({"package": "app", "stage": "verify", "index": 2, "count": 2})),
        push(json!({"package": "lib", "stage": "install", "index": 1, "count": 2})),
        push(json!({"package": "app", "stage": "install", "index": 2, "count": 2})),
    ]
}

fn printed(language: &Language, terminal: Option<usize>) -> String {
    let packages = [
        ("lib".to_string(), "1.0".to_string()),
        ("app".to_string(), "1.0".to_string()),
    ];
    let mut progress = Progress::new(language, terminal, &packages);
    let mut err: Vec<u8> = Vec::new();
    for message in pushes() {
        progress.on(&message, &mut err);
    }
    progress.finish(&mut err);
    String::from_utf8(err).expect("是 UTF-8")
}

#[test]
fn a_pipe_gets_one_line_per_step_and_no_bar() {
    assert_eq!(
        printed(&Language::Chinese, None),
        "正在下载…\n已下载 lib-1.0 1000 B\n已下载 app-1.0 2.0 KiB\n(1/2) 正在核对完整性\n(2/2) 正在核对完整性\n\
         (1/2) 正在安装 lib\n(2/2) 正在安装 app\n"
    );
    assert_eq!(
        printed(&Language::English, None),
        "downloading packages...\ndownloaded lib-1.0 1000 B\ndownloaded app-1.0 2.0 KiB\n\
         (1/2) checking package integrity\n(2/2) checking package integrity\n(1/2) installing lib\n(2/2) installing app\n"
    );
}

#[test]
fn a_terminal_redraws_the_bar_and_the_integrity_line_in_place() {
    let redraw = "\r\x1b[2K";
    let lib = |done| bar(("lib-1.0", 7), (done, 1000), 80);
    let app = |done| bar(("app-1.0", 7), (done, 2048), 80);
    assert_eq!(
        printed(&Language::Chinese, Some(80)),
        format!(
            "正在下载…\n{redraw}{}{redraw}{}\n{redraw}{}{redraw}{}\n{redraw}(1/2) 正在核对完整性{redraw}(2/2) 正在核对完整性\n\
             (1/2) 正在安装 lib\n(2/2) 正在安装 app\n",
            lib(0),
            lib(1000),
            app(0),
            app(2048)
        )
    );
}

#[test]
fn a_bar_left_half_drawn_ends_its_line_when_the_install_ends() {
    let language = Language::Chinese;
    let mut progress = Progress::new(&language, Some(80), &[]);
    let mut err: Vec<u8> = Vec::new();
    let push =
        json!({"method": "package.progress", "params": {"package": "x", "done": 1, "total": 2}});
    progress.on(&push, &mut err);
    progress.finish(&mut err);
    let text = String::from_utf8(err).expect("是 UTF-8");
    assert!(text.ends_with("50%\n"), "{text:?}");
}
