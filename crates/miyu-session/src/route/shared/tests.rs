//! 供应商列表的两样（施工 V-26）：后台拉一家有间隔；换上的列表真变了才交「这一家变了」。

use std::time::Duration;

use miyu_kernel::time::Timestamp;
use miyu_models::matching::Vendors;
use miyu_models::observed::{ListedModel, ProviderList};
use miyu_models::profile::Profiles;

use super::ModelData;

fn data() -> ModelData {
    ModelData::new(Profiles::default(), Vendors::default(), None)
}

/// 列着 `ids` 的一份，拉的时刻是 `at` 毫秒。
fn list(ids: &[&str], at: i64) -> ProviderList {
    ProviderList {
        fetched: Timestamp::from_unix_millis(at).expect("合写法"),
        models: ids
            .iter()
            .map(|id| ListedModel {
                id: (*id).to_string(),
                window: None,
            })
            .collect(),
    }
}

/// 一家一段时间里只在后台拉一次；别的家照拉；过了间隔照拉。
#[test]
fn a_provider_is_claimed_once_per_interval() {
    let data = data();
    let minute = Duration::from_secs(60);
    assert!(data.claim_list("deepseek", minute));
    assert!(!data.claim_list("deepseek", minute), "一分钟里不再拉");
    assert!(data.claim_list("openai", minute), "别的家照拉");
    assert!(data.claim_list("deepseek", Duration::ZERO), "过了间隔照拉");
}

/// 头一次拉到、列的模型变了才交；只差拉的时刻的不交。
#[test]
fn only_a_changed_list_is_announced() {
    let data = data();
    let mut changes = data.list_changes();
    let deepseek = Some(vec!["deepseek".to_string()]);
    data.set_list("deepseek", list(&["a", "b"], 1));
    assert_eq!(changes.try_recv().ok(), deepseek, "头一次拉到");
    data.set_list("deepseek", list(&["a", "b"], 2));
    assert!(changes.try_recv().is_err(), "一样的不交");
    data.set_list("deepseek", list(&["a", "c"], 3));
    assert_eq!(changes.try_recv().ok(), deepseek, "变了");
    data.set_list("deepseek", list(&["c", "a"], 4));
    assert_eq!(changes.try_recv().ok(), deepseek, "先后变了也算");
}

/// 用出来的和拉到的分两处写（施工 FH-2，设计 55）：学到的窗口、池的指针在 `state`，供应商的列表在 `cache`；读回来照样两处读。
#[test]
fn what_is_learned_and_what_is_fetched_are_kept_apart() {
    let scratch = std::env::temp_dir().join(format!("miyu-model-dirs-{}", std::process::id()));
    let root = miyu_store::root::DataRoot::at(scratch.clone());
    std::fs::create_dir_all(&scratch).expect("建得了");
    std::fs::write(
        scratch.join(miyu_store::root::MARKER),
        miyu_store::root::MARKER_TEXT,
    )
    .expect("写得进");
    let dirs = super::ModelDirs {
        state: root.path().join("learned"),
        cache: root.path().join("fetched"),
    };
    let data = ModelData::new(Profiles::default(), Vendors::default(), Some(dirs.clone()));
    let at = Timestamp::from_unix_millis(1).expect("合写法");
    data.learn("dev", "m", 4096, at);
    data.set_list("dev", list(&["m"], 1));
    data.save_pointers();
    assert!(dirs.state.join("learned.json").is_file());
    assert!(dirs.state.join("pools.json").is_file());
    assert!(dirs.cache.join("providers/dev.json").is_file());
    assert!(
        !dirs.state.join("providers").exists(),
        "列表不在用出来的那一处"
    );
    assert!(
        !dirs.cache.join("learned.json").exists(),
        "学到的不在拉到的那一处"
    );
    let read = super::read_observed(&dirs);
    assert_eq!(read.lists.len(), 1);
    assert!(read.learned.window("dev", "m").is_some());
    drop(std::fs::remove_dir_all(&scratch));
}
