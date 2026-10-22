//! 认出来（施工 E-1 补）：和她说话的是开口的人里最后一个不是主人的；这一段带过谁照 `present` 里模块 `memory`、类别 `card` 的
//! `refs` 算；一块逐字节对。

use std::path::Path;

use miyu_kernel::facts::{Present, Speaker};
use miyu_kernel::id::{ExternalId, FactKind, ModuleId};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;
use miyu_recall::PersonCard;

use super::super::SummaryTexts;
use super::{brought, render, talking};

fn speaker(id: &str, owner: bool) -> Speaker {
    Speaker {
        id: ExternalId::parse(id).unwrap(),
        owner,
    }
}

fn present(module: &str, kind: &str, refs: &[&str]) -> Present {
    Present {
        module: ModuleId::parse(module).unwrap(),
        kind: FactKind::parse(kind).unwrap(),
        refs: refs.iter().map(ToString::to_string).collect(),
    }
}

#[test]
fn the_one_talking_is_the_last_who_is_not_the_owner() {
    let (a, b, owner) = (
        speaker("qq:1", false),
        speaker("qq:2", false),
        speaker("qq:9", true),
    );
    assert_eq!(
        talking(&[a.clone(), b.clone()]),
        Some("qq:2"),
        "最近开口的在后"
    );
    assert_eq!(
        talking(&[a, b.clone(), owner.clone()]),
        Some("qq:2"),
        "主人不算"
    );
    assert_eq!(talking(&[owner]), None);
    assert_eq!(talking(&[]), None, "本机、不是人开的一轮没有");
}

#[test]
fn what_was_brought_counts_only_cards_of_this_module() {
    let present = [
        present("memory", "card", &["qq:1"]),
        present("memory", "recall", &["qq:2"]),
        present("roleplay", "card", &["qq:3"]),
    ];
    assert!(brought(&present, "qq:1"));
    assert!(!brought(&present, "qq:2"), "别的类别不算");
    assert!(!brought(&present, "qq:3"), "别的模块不算");
    assert!(!brought(&present, "qq:4"));
}

#[test]
fn a_block_is_exact() {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let texts = SummaryTexts::load(&resources).expect("读得到");
    let mut card = PersonCard {
        who: "qq:2".into(),
        text: "小林喜欢拍照。\n下周搬去大阪。".into(),
        names: vec!["小林".into(), "林林".into()],
        at: Timestamp::parse("2026-10-22T03:00:00.000Z").unwrap(),
        by: By::Kernel,
        retired: None,
    };
    let block = render(&texts.card, &card);
    assert_eq!(block.module.as_str(), "memory");
    assert_eq!(block.fact.kind.as_str(), "card");
    assert_eq!(block.fact.refs, ["qq:2"]);
    assert!(!block.fact.after);
    assert_eq!(
        block.fact.text,
        "<person>\nTalking to you: qq:2 (小林, 林林)\n小林喜欢拍照。 下周搬去大阪。\n</person>\n"
    );
    card.names.clear();
    assert_eq!(
        render(&texts.card, &card).fact.text,
        "<person>\nTalking to you: qq:2\n小林喜欢拍照。 下周搬去大阪。\n</person>\n",
        "没有叫法的只写是谁"
    );
}
