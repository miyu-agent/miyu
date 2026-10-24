use std::sync::Arc;

use miyu_kernel::tool::Access;

use super::*;
use crate::testkit::{Act, Fake};

fn object(name: &str) -> Arc<dyn Tool> {
    Fake::new(name, Access::Read, Act::Echo)
}

fn names(catalog: &Catalog) -> Vec<String> {
    catalog.specs().map(|spec| spec.name.clone()).collect()
}

/// 换一次是一代：拿着同一个架子的都看到新的；换不成的不动、不算一代。
#[test]
fn a_shelf_hands_out_the_current_catalog_and_counts_editions() {
    let shelf = Shelf::new(Catalog::new([object("read")]).unwrap());
    let held = shelf.clone();
    let first = shelf.edition();
    assert_eq!(names(&first.catalog), ["read"]);
    let generation = shelf
        .replace(|catalog| catalog.replacing("onebot", vec![object("send")]))
        .unwrap();
    assert_eq!(generation, first.generation + 1);
    assert_eq!(
        names(&held.current()),
        ["read", "send"],
        "拿着同一个架子的看到新的"
    );
    assert!(held.get("send").is_some());
    assert!(held.get("nothing").is_none());
    let refused = shelf.replace(|catalog| catalog.replacing("onebot", vec![object("read")]));
    assert!(refused.is_err());
    let now = shelf.edition();
    assert_eq!(now.generation, generation, "换不成的不算一代");
    assert_eq!(names(&now.catalog), ["read", "send"], "换不成的不动");
}

/// 叠起来的架子（施工 U23 中中）：照底下那一份拼；底下换了、自己那一层换了，拼好的那份都作废、当场重拼，代数只增不减；从叠起来的
/// 换，换的是底下那一个。
#[test]
fn a_shelf_over_another_follows_both_and_counts_up() {
    let base = Shelf::new(
        Catalog::in_packages([
            ("basesystem", vec![object("read")]),
            ("~home", vec![object("weather")]),
        ])
        .unwrap(),
    );
    let mine = Shelf::over(
        &base,
        Arc::new(|catalog: &Catalog| catalog.without("~home")),
    );
    let first = mine.edition();
    assert_eq!(names(&first.catalog), ["read"], "照底下拼：拿掉 ~home");
    assert!(mine.get("weather").is_none());
    assert_eq!(names(&base.current()), ["read", "weather"], "底下的不动");
    // 底下换了：拼好的作废。
    base.replace(|catalog| catalog.replacing("onebot", vec![object("send")]))
        .unwrap();
    let second = mine.edition();
    assert_eq!(names(&second.catalog), ["read", "send"], "底下换了的跟着换");
    assert!(second.generation > first.generation, "代数往上走");
    // 自己那一层换了：拼好的作废。
    assert!(mine.recompose(Arc::new(|catalog: &Catalog| catalog.clone())));
    let third = mine.edition();
    assert_eq!(
        names(&third.catalog),
        ["read", "send", "weather"],
        "自己那一层换了的跟着换"
    );
    assert!(third.generation > second.generation, "代数往上走");
    assert_eq!(
        mine.edition().generation,
        third.generation,
        "都没变的不算一代"
    );
    // 从叠起来的换：换的是底下那一个。
    mine.replace(|catalog| catalog.replacing("onebot", Vec::new()))
        .unwrap();
    assert_eq!(names(&base.current()), ["read", "weather"]);
    assert!(mine.edition().generation > third.generation);
    assert!(
        !base.recompose(Arc::new(|catalog: &Catalog| catalog.clone())),
        "自己放着的不叠"
    );
}
