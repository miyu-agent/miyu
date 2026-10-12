//! 场景：用过的技能压后重建（施工 X-10 下）。第一轮她读了两个文件和一个技能的 `SKILL.md`，第二轮一开头就压：技能另算
//! 一组，不占文件的名额，接在文件后面重读、挑；单个、合计照技能的数；太大的进清单。以前造的快照没有技能那一组：技能照
//! 普通文件算。

use super::*;
use crate::event::{ContextCompacted, RestoredFile};
use crate::id::ContentHash;
use crate::session::{Compaction, Notes, Rebuild, SkillRebuild};
use crate::template::Template;

const SKILL: &str = "/d/skills/notes/SKILL.md";

/// 会压缩、会重读的替身：最多重读 1 个文件，单个 20 token、合计 30，候选 3 个；技能那一组照 `skills`。
fn rebuilding(skills: Option<SkillRebuild>) -> Stage {
    let make = move || {
        let mut policy = policy();
        policy.compaction = Some(Compaction {
            reserve_cap: 10,
            margin: 10,
            line_percent: 100,
            margin_percent: 100,
            tail: 0,
            lead: 0,
            price: crate::estimate::Flat {
                image: 50,
                file: 50,
            },
            rebuild: Some(Rebuild {
                files: 1,
                file_tokens: 20,
                total: 30,
                min_window: 100,
                candidates: 3,
                skills,
            }),
            pause: None,
            shorten: None,
            isolate: false,
        });
        let template = |source: &str| Template::parse(source).unwrap();
        policy.notes = Some(Notes {
            files: template("<files>\n"),
            files_more: template("<more {count}/>\n"),
            retrieve: template("<retrieve {upto}/>\n"),
            too_large: template("<too-large {files}/>\n"),
            todos: None,
            uncovered: None,
        });
        policy
    };
    Stage::new(make, environment("/w"), at(0))
}

/// 第一轮依次读 a.rs、技能、b.rs（技能的正文是 `skill`），再答一句（报 5000）；第二轮一开头就压。
fn read_then_compact(stage: &mut Stage, skill: &str) {
    let calls = [
        ("read", r#"{"path":"a.rs"}"#),
        ("read", r#"{"path":"/d/skills/notes/SKILL.md"}"#),
        ("read", r#"{"path":"b.rs"}"#),
    ];
    stage.model([
        Line::calls("看看。", &calls),
        Line::says("好。").reports(5_000),
    ]);
    stage.tools([
        Play::read("/w/a.rs", "a"),
        Play::skill(SKILL, skill, "notes"),
        Play::read("/w/b.rs", "b"),
    ]);
    stage.disk("/w/a.rs", "a");
    stage.disk("/w/b.rs", "b");
    stage.disk(SKILL, skill);
    stage.say("读一下");
    stage.limits(Some(120), None);
    stage.model([Line::says("S1"), Line::says("嗯。")]);
    stage.say("接着来");
}

fn compacted(stage: &Stage) -> &ContextCompacted {
    stage
        .log()
        .iter()
        .find_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => Some(compacted),
            _ => None,
        })
        .expect("压了")
}

fn restored(compacted: &ContextCompacted) -> Vec<&str> {
    compacted
        .restored
        .iter()
        .map(|file| file.path.as_str())
        .collect()
}

#[test]
fn a_used_skill_is_reread_in_its_own_group_after_the_files() {
    let mut stage = rebuilding(Some(SkillRebuild {
        tokens: 20,
        total: 20,
    }));
    read_then_compact(&mut stage, "steps");
    let rereads = stage.rereads();
    assert_eq!(
        rereads[0].1,
        ["/w/b.rs", "/w/a.rs", SKILL],
        "技能不占文件的候选"
    );
    let compacted = compacted(&stage);
    // 文件的名额只有 1 个，技能照样重读，接在文件后面。
    assert_eq!(restored(compacted), ["b.rs", SKILL]);
    assert_eq!(
        compacted.restored[1],
        RestoredFile {
            path: SKILL.to_string(),
            blob: ContentHash::of(b"steps"),
            tokens: 2,
        }
    );
}

#[test]
fn a_skill_over_its_own_limit_is_listed_instead() {
    let mut stage = rebuilding(Some(SkillRebuild {
        tokens: 20,
        total: 10,
    }));
    // 16 个 token：单个不超过 20，合计超过 10。
    read_then_compact(&mut stage, &"s".repeat(64));
    let compacted = compacted(&stage);
    assert_eq!(restored(compacted), ["b.rs"]);
    assert!(
        compacted
            .notes
            .ends_with(&format!("<too-large {SKILL}/>\n")),
        "{}",
        compacted.notes
    );
}

#[test]
fn without_the_skill_numbers_a_skill_counts_as_a_file() {
    let mut stage = rebuilding(None);
    read_then_compact(&mut stage, "steps");
    assert_eq!(stage.rereads()[0].1, ["/w/b.rs", SKILL, "/w/a.rs"]);
    assert_eq!(restored(compacted(&stage)), ["b.rs"], "文件的名额只有 1 个");
}

/// 技能的单个上限比文件的大：交给执行器的单个上限照大的那个折成字节，比文件上限大的技能照样读得回来。
#[test]
fn the_reread_limit_is_the_larger_of_the_two() {
    let mut stage = rebuilding(Some(SkillRebuild {
        tokens: 40,
        total: 40,
    }));
    // 30 个 token：超过文件的 20，不超过技能的 40。
    read_then_compact(&mut stage, &"s".repeat(120));
    assert_eq!(stage.rereads()[0].2, 160, "40 token 折成 160 字节");
    assert_eq!(restored(compacted(&stage)), ["b.rs", SKILL]);
}
