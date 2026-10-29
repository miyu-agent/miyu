//! 知识库只照意思找到的那一路的下限照模型（施工 K-2 修，`docs/blueprint/knowledge.md` 第四条）：门槛表里有的照它，没有的照
//! 0.40（`miyu_recall::FLOOR`）；出厂的表里 granite 是 0.82（bge 那一行施工 R-18 删掉了）。

use std::path::{Path, PathBuf};

use miyu_recall::associate::Thresholds;
use miyu_store::knowledge::{Knowledge, Meaning};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::support::*;

fn kb(root: &DataRoot, id: &str) -> PathBuf {
    let dir = root.path().join("kb").join(id);
    std::fs::create_dir_all(&dir).expect("建得了");
    dir
}

fn write(dir: &Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).expect("写得进");
}

#[test]
fn the_floor_follows_the_model() {
    let scratch = Scratch::new("kb-floor");
    let root = root_in(&scratch);
    write(&kb(&root, "notes"), "cat.md", "团子的饭是鸡胸肉\n");
    let floors = Thresholds::parse("[thresholds]\n\"m\" = 0.5\n").expect("合写法");
    let knowledge = Knowledge::new(&root).with_floors(floors);
    knowledge.list().expect("补得了");
    let index = knowledge.index_of("notes");
    for model in ["m", "n"] {
        index
            .put_vector("cat.md#0", model, &[1.0, 0.0])
            .expect("写得进");
    }
    let found = |model: &str, similar: f32| {
        let vector = [similar, (1.0 - similar * similar).sqrt()];
        let near = Meaning {
            model,
            vector: &vector,
        };
        !knowledge
            .search("猫吃什么", None, 5, Some(near))
            .expect("搜得了")
            .0
            .is_empty()
    };
    assert!(found("m", 0.55), "过了表里的 0.5");
    assert!(!found("m", 0.45), "表里的模型照 0.5：0.45 不要");
    assert!(found("n", 0.45), "表里没有的照 0.40");
    assert!(!found("n", 0.35));
}

#[test]
fn the_shipped_table_has_granite_and_no_bge() {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let text = ResourceRoot::at(&resources)
        .knowledge_table("search")
        .expect("出厂的读得出来");
    let floors = Thresholds::parse(&text).expect("合写法");
    assert_eq!(
        floors.of("local:bge-small-zh-v1.5"),
        None,
        "bge 删掉了（施工 R-18）"
    );
    assert_eq!(
        floors.of("local:granite-embedding-97m-multilingual-r2"),
        Some(0.82),
        "出厂模型换成 granite（施工 R-5 七补）"
    );
}
