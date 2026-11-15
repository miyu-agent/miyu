//! 记字体位置的目录不会被别的家底改走（施工 RB-1）：画图的库记字体位置的目录整个进程只有一个，一个进程里有几份核心（测试里）时，
//! 一份正在画、头一回读系统字体库，另一份这时定成自己的目录，位置就记进了别人的目录；那一份的测试早就删了它的数据根，这一写又建
//! 回 `var/cache/mermaid/<哈希>.location`。这里另起一个测试程序：画图的库只在这个进程头一回读某串字体时记位置，别的测试先画过
//! 就看不到了。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use miyu_mermaid::Mermaid;

const FLOWCHART: &str =
    "flowchart TD\n  A[Start] --> B{Decision}\n  B -->|Yes| C[OK]\n  B -->|No| D[Cancel]";

/// 一个用完就删的临时目录：资源目录里写好 `software/mermaid/style.json`。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("miyu-mermaid-font-dir-{}", std::process::id()));
        let style = dir.join("resources").join("software").join("mermaid");
        std::fs::create_dir_all(&style).expect("建得了目录");
        let mut file = std::fs::File::create(style.join("style.json")).expect("建得了文件");
        file.write_all(
            br##"{"fonts":["sans-serif"],"marks":{"text":"#010203","line":"#040506","label":"#070809"},"max_source":65536,"keep":64}"##,
        )
        .expect("写得进");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `dir` 里有没有记下的字体位置。
fn located(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|ext| ext == "location")
        })
    })
}

#[test]
fn a_render_remembers_its_fonts_only_in_its_own_directory() {
    let scratch = Scratch::new();
    let resources = scratch.0.join("resources");
    let (mine, theirs) = (scratch.0.join("mine"), scratch.0.join("theirs"));
    let drawing = Mermaid::new(&resources).with_font_cache(mine.clone());
    // 另一份一直在画读不懂的源码：每一次都先把目录定成自己的，再在读源码时失败，自己从不读字体。
    let other = Arc::new(Mermaid::new(&resources).with_font_cache(theirs.clone()));
    let done = Arc::new(AtomicBool::new(false));
    let spinning = {
        let (other, done) = (Arc::clone(&other), Arc::clone(&done));
        std::thread::spawn(move || {
            while !done.load(Ordering::Acquire) {
                assert!(other.render("not a diagram at all").is_err(), "读不懂");
            }
        })
    };
    let drawn = drawing.render(FLOWCHART);
    done.store(true, Ordering::Release);
    spinning.join().expect("那一头没崩");
    drawn.expect("画得出来：这台机器上要有字体");
    assert!(!located(&theirs), "没画过图的那一份目录里不该有位置记录");
    assert!(located(&mine), "记在画图的这一份自己的目录里");
}
