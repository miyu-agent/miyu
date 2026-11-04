//! 整个会话喂投影对照整份读（施工 SE-1 中下上）：照翻页对照测试的造法（`view/read/tests/same.rs` 的 `made`：一段到几段、
//! 中间夹空段、末尾半行、分叉出来从中间的序号起）造会话日志，边读边喂的和整份读成事件再喂的，条目一样、交给 `each` 的事件
//! 一样、最后一条一样。

use miyu_kernel::id::Seq;
use miyu_store::blob::Blobs;
use miyu_store::log::{OpenError, read_events};

use super::whole;
use crate::test_support::temp_root;
use crate::view::project;
use crate::view::read::tests::same::{made, texts};

#[test]
fn feeding_as_it_reads_gives_the_entries_of_feeding_the_whole_log() {
    let (root, place) = temp_root("se1-whole");
    let blobs = Blobs::new(root.blobs());
    let texts = texts();
    for seed in 0..40 {
        let dir = place.join(format!("s{seed}"));
        made(seed, &dir);
        let events = read_events(&dir).expect("造的日志读得成");
        let mut whole_projector = project::fresh(texts.clone(), blobs.clone());
        for event in &events {
            whole_projector.event(event);
        }
        let mut seen = Vec::new();
        let read = whole(&dir, texts.clone(), blobs.clone(), |event| {
            seen.push(event.seq)
        });
        let Some(last) = events.last() else {
            assert!(
                matches!(read, Err(OpenError::Missing(_))),
                "seed {seed}：一条都没有的当没有这个会话"
            );
            continue;
        };
        let read = read.unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        assert_eq!(
            read.projector.entries(),
            whole_projector.entries(),
            "seed {seed}"
        );
        let all: Vec<Seq> = events.iter().map(|event| event.seq).collect();
        assert_eq!(seen, all, "seed {seed}：每一条照先后交给 each");
        assert_eq!(read.upto, last.seq, "seed {seed}");
        miyu_store::testkit::remove_tree(&dir);
    }
}
