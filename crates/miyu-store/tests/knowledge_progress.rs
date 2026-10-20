//! 建索引的进度（施工 W-15，`knowledge/watch.rs`）：照文件补的时候要重建的一篇篇报，开头 0、最后一篇照报，完了报建好了，记下
//! 上次建好的时刻；没有要重建的什么都不报；别处正在补的库，列出来不等它，带 `indexing`。跳过的文档没变的不重读、不报进度，
//! 照报跳过（施工 W-15 再补）。

use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use miyu_kernel::id::AccountId;
use miyu_store::knowledge::{BaseFields, IndexWatch, Knowledge, Reason};

use crate::support::*;

/// 记下报来的；`hold` 有的，头一回报进度时先说一声、等放行（好在补到一半时插手）。
#[derive(Default)]
struct Recorder {
    heard: Mutex<Vec<String>>,
    hold: Mutex<Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>>,
}

impl IndexWatch for Recorder {
    fn indexing(&self, _: &AccountId, kb: &str, done: usize, total: usize) {
        self.heard
            .lock()
            .expect("拿得到锁")
            .push(format!("{kb} {done}/{total}"));
        if let Some((started, go)) = self.hold.lock().expect("拿得到锁").take() {
            started.send(()).expect("有人等着");
            go.recv().expect("放行");
        }
    }

    fn indexed(&self, _: &AccountId, kb: &str) {
        self.heard
            .lock()
            .expect("拿得到锁")
            .push(format!("{kb} indexed"));
    }
}

/// 一个库 `notes`，三篇。
fn filled(scratch: &Scratch, recorder: &Arc<Recorder>) -> Knowledge {
    let root = root_in(scratch);
    let knowledge = Knowledge::new(&root).with_watch(Arc::clone(recorder) as Arc<dyn IndexWatch>);
    let none = BaseFields {
        name: None,
        description: None,
        auto: None,
    };
    knowledge.found(&admin(), "notes", none).expect("建得了");
    let dir = root.account_dir(&admin()).join("kb/notes");
    for name in ["a.md", "b.md", "c.md"] {
        std::fs::write(dir.join(name), format!("# {name}\n猫\n")).expect("写得进");
    }
    knowledge
}

#[test]
fn rebuilding_is_reported_and_remembered() {
    let scratch = Scratch::new("kb-progress");
    let recorder = Arc::new(Recorder::default());
    let knowledge = filled(&scratch, &recorder);
    let (bases, _) = knowledge.list(&admin()).expect("列得出");
    let heard = recorder.heard.lock().expect("拿得到锁").clone();
    assert_eq!(heard.first().map(String::as_str), Some("notes 0/3"));
    assert_eq!(
        heard[heard.len() - 2],
        "notes 3/3",
        "最后一篇照报：{heard:?}"
    );
    assert_eq!(heard.last().map(String::as_str), Some("notes indexed"));
    assert!(
        bases[0]
            .indexed_at
            .as_deref()
            .is_some_and(|at| at.ends_with('Z'))
    );
    assert!(!bases[0].indexing);

    let before = recorder.heard.lock().expect("拿得到锁").len();
    knowledge.list(&admin()).expect("列得出");
    assert_eq!(
        recorder.heard.lock().expect("拿得到锁").len(),
        before,
        "没有要重建的不报"
    );
}

#[test]
fn a_base_being_built_elsewhere_is_listed_without_waiting() {
    let scratch = Scratch::new("kb-progress-busy");
    let recorder = Arc::new(Recorder::default());
    let (started, wait) = mpsc::channel();
    let (go, gate) = mpsc::channel();
    *recorder.hold.lock().expect("拿得到锁") = Some((started, gate));
    let knowledge = Arc::new(filled(&scratch, &recorder));
    let building = {
        let knowledge = Arc::clone(&knowledge);
        std::thread::spawn(move || knowledge.reindex(&admin(), "notes").expect("建得了"))
    };
    wait.recv().expect("开始建了");
    let (bases, _) = knowledge.list(&admin()).expect("不等它");
    assert!(bases[0].indexing, "在建：{bases:?}");
    go.send(()).expect("放行");
    building.join().expect("建完了");
    let (bases, _) = knowledge.list(&admin()).expect("列得出");
    assert!(!bases[0].indexing);
}

#[test]
fn an_unchanged_skipped_document_is_not_read_again() {
    let scratch = Scratch::new("kb-progress-skipped");
    let recorder = Arc::new(Recorder::default());
    let knowledge = filled(&scratch, &recorder);
    let dir = root_in(&scratch).account_dir(&admin()).join("kb/notes");
    std::fs::write(dir.join("latin1.txt"), [b'c', b'a', 0xe9, b'\n']).expect("写得进");
    let (_, skipped) = knowledge.list(&admin()).expect("列得出");
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    assert_eq!(skipped[0].reason, Reason::NotUtf8);

    let before = recorder.heard.lock().expect("拿得到锁").len();
    let (_, again) = knowledge.list(&admin()).expect("列得出");
    assert_eq!(
        recorder.heard.lock().expect("拿得到锁")[before..],
        [] as [String; 0],
        "没变的跳过不重读、不报"
    );
    assert_eq!(again.len(), 1, "照报跳过：{again:?}");
    assert_eq!(again[0].path, skipped[0].path);
    assert_eq!(again[0].reason, Reason::NotUtf8);
    assert_eq!(again[0].why, skipped[0].why);

    std::fs::write(dir.join("latin1.txt"), "café 猫\n").expect("写得进");
    let (_, fixed) = knowledge.list(&admin()).expect("列得出");
    assert!(fixed.is_empty(), "改好了重读：{fixed:?}");
    assert!(
        recorder.heard.lock().expect("拿得到锁")[before..]
            .iter()
            .any(|heard| heard == "notes 1/1")
    );
}
