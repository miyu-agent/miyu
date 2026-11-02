//! 读好的字放一阵（施工 V-30）：文件没变只读一遍；改了、多出一份、多一个包的照旧重读；拷出来的资源目录共用；放掉以后
//! 再读；读不懂的不放。

use std::cell::Cell;
use std::fs;
use std::path::Path;

use miyu_kernel::event::Said;

use crate::human::{Human, places};
use crate::resources::ResourceRoot;
use crate::test_support::Scratch;

/// 在 `dir` 下写一份 `language` 的字，说法 `hello` 是 `text`。
fn write_words(dir: &Path, language: &str, text: &str) {
    let human = dir.join("human");
    fs::create_dir_all(&human).unwrap();
    let body = serde_json::json!({"said": {"hello": text}});
    fs::write(human.join(format!("{language}.json")), body.to_string()).unwrap();
}

fn hello(human: &Human) -> Option<String> {
    human.say(&Said::new("core/hello"))
}

/// 照架子取一份，读的时候记一次。
fn take(root: &ResourceRoot, language: &str, reads: &Cell<usize>) -> Option<String> {
    let places = places(root);
    let human = root
        .shelf
        .get(language, &places, |language| {
            reads.set(reads.get() + 1);
            Human::read(&places, language)
        })
        .unwrap();
    hello(&human)
}

#[test]
fn unchanged_words_are_read_once_and_changes_are_read_again() {
    let scratch = Scratch::new();
    let core = scratch.path().join("core");
    write_words(&core, "en", "first");
    let root = ResourceRoot::at(scratch.path());
    let reads = Cell::new(0);
    assert_eq!(take(&root, "en", &reads).as_deref(), Some("first"));
    assert_eq!(take(&root, "en", &reads).as_deref(), Some("first"));
    // 拷出来的资源目录共用同一个架子。
    assert_eq!(take(&root.clone(), "en", &reads).as_deref(), Some("first"));
    assert_eq!(reads.get(), 1, "文件没变只读一遍");
    // 改了资源：核心不重启，下一次就是新的（施工 W-1）。
    write_words(&core, "en", "second, longer");
    assert_eq!(take(&root, "en", &reads).as_deref(), Some("second, longer"));
    assert_eq!(reads.get(), 2);
    // 放掉以后再读。
    assert!(root.clone().forget_words(), "放着一份");
    assert!(!root.forget_words(), "已经放掉了");
    assert_eq!(take(&root, "en", &reads).as_deref(), Some("second, longer"));
    assert_eq!(reads.get(), 3);
}

#[test]
fn a_new_language_file_or_package_is_read_again() {
    let scratch = Scratch::new();
    let core = scratch.path().join("core");
    write_words(&core, "en", "english");
    let root = ResourceRoot::at(scratch.path());
    let reads = Cell::new(0);
    // 没有日文的照英文，放的就是英文那一份。
    assert_eq!(take(&root, "ja", &reads).as_deref(), Some("english"));
    // 原来没有的日文那份放进去了：看得出来。
    write_words(&core, "ja", "日本語");
    assert_eq!(take(&root, "ja", &reads).as_deref(), Some("日本語"));
    assert_eq!(reads.get(), 2);
    // 多了一个出厂包：看得出来。
    write_words(&scratch.path().join("software").join("extra"), "en", "pkg");
    take(&root, "ja", &reads);
    assert_eq!(reads.get(), 3);
    // 第一步没有日文时照英文放过一份（审计 R3E-6），多了包看得出来，重读。
    assert_eq!(take(&root, "en", &reads).as_deref(), Some("english"));
    assert_eq!(reads.get(), 4);
}

#[test]
fn unreadable_words_are_not_held() {
    let scratch = Scratch::new();
    let human = scratch.path().join("core").join("human");
    fs::create_dir_all(&human).unwrap();
    fs::write(human.join("en.json"), "{not json").unwrap();
    let root = ResourceRoot::at(scratch.path());
    assert!(Human::load(&root, "en").is_err(), "读不懂照报");
    assert!(
        Human::load(&root, "en").is_err(),
        "没放上去，照旧再读、照旧报"
    );
    write_words(&scratch.path().join("core"), "en", "fixed");
    assert_eq!(
        hello(&Human::load(&root, "en").unwrap()).as_deref(),
        Some("fixed")
    );
}

/// 审计 2026-11-02 第三轮（边缘）：架子照要的语言放，`human.get` 收 2 到 8 个小写字母的任何一种。没有自己那份文件的语言读出来
/// 都是英文那份，却各占一份完整的 [`Human`]；只有核心安静下来（2 秒没有请求）才放掉，一直有请求、换着语言问的，放着的跟着涨。
#[test]
fn languages_without_their_own_words_do_not_each_hold_a_copy() {
    let scratch = Scratch::new();
    let core = scratch.path().join("core");
    write_words(&core, "en", "hello");
    write_words(&core, "zh", "你好");
    let root = ResourceRoot::at(scratch.path());
    for n in 0..200u32 {
        let language: String = n
            .to_string()
            .chars()
            .map(|digit| char::from(b'a' + digit.to_digit(10).map_or(0, |d| d as u8)))
            .chain("xq".chars())
            .collect();
        let human = Human::load(&root, &language).unwrap();
        assert_eq!(hello(&human).as_deref(), Some("hello"), "{language} 照英文");
    }
    let held = root.shelf.lock().len();
    assert!(
        held <= 3,
        "200 种没有自己那份字的语言，架子上放了 {held} 份"
    );
}
