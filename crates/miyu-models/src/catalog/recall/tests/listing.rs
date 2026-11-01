//! 目录里一家的模型清单（施工 V-32，`listing_side`）：照记下的答和整份目录现算逐字一样（边查边记、只照记下的、写成 JSON 读
//! 回来三遍）；说不出来的立一下；换了目录照新目录重算；两份并起来照先有的答；路由那份没有清单的，写出来的字没有这一格，
//! 以前写的照样读得回来。

use super::rebuild::changed;
use super::*;
use crate::catalog::listing_side;

/// 问清单的输入：目录里每一家的编号，加几个没有的。
fn listing_asks(catalog: &Catalog) -> Vec<String> {
    let mut asks: Vec<String> = catalog
        .providers()
        .map(|listed| listed.id.clone())
        .collect();
    asks.extend(["nope", "DeepSeek", ""].map(str::to_string));
    asks
}

#[test]
fn a_recalled_listing_answers_like_the_full_catalog() {
    let loaded = loaded();
    let asks = listing_asks(&loaded.catalog);
    let recalled = Mutex::new(Recalled::new("trimmed 1", loaded.source, &loaded.fetched));
    let lookup = |catalog| {
        CatalogLookup::Recalled(Recalling {
            recalled: &recalled,
            catalog,
            remember: true,
            missed: None,
        })
    };
    let full = CatalogLookup::Full(&loaded);
    let mut listed = 0;
    for ask in &asks {
        let expected = full.listing(ask);
        listed += usize::from(expected.as_ref().is_some_and(|it| !it.models.is_empty()));
        same(&expected, &lookup(Some(&loaded)).listing(ask), &|| {
            format!("边查边记：{ask}")
        });
        same(&expected, &lookup(None).listing(ask), &|| {
            format!("只照记下的：{ask}")
        });
    }
    assert!(listed > 5, "要真有几家列得出模型");
    assert_eq!(full.listing("nope"), None, "目录里没有的一家没有清单");
    let deepseek = full.listing("deepseek").expect("目录里有 deepseek");
    assert_eq!(deepseek.name.as_deref(), Some("DeepSeek"));
    assert!(deepseek.models.values().any(Option::is_some), "带着 family");
    let text = locked(&recalled).to_json().expect("写得成");
    assert!(text.contains("\"listings\""));
    let back = Mutex::new(Recalled::from_json(&text).expect("读得回来"));
    let reread = CatalogLookup::Recalled(Recalling {
        recalled: &back,
        catalog: None,
        remember: false,
        missed: None,
    });
    for ask in &asks {
        same(&full.listing(ask), &reread.listing(ask), &|| {
            format!("读回来的：{ask}")
        });
    }
}

#[test]
fn an_unknown_listing_raises_the_missed_flag() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let loaded = loaded();
    let recalled = Mutex::new(Recalled::new("trimmed 1", loaded.source, &loaded.fetched));
    let flag = AtomicBool::new(false);
    let lookup = |catalog, remember| {
        CatalogLookup::Recalled(Recalling {
            recalled: &recalled,
            catalog,
            remember,
            missed: Some(&flag),
        })
    };
    // 不记的借得到目录也答得出来，不留。
    assert!(lookup(Some(&loaded), false).listing("deepseek").is_some());
    assert!(!flag.load(Ordering::Relaxed));
    assert_eq!(lookup(None, false).listing("deepseek"), None);
    assert!(flag.swap(false, Ordering::Relaxed), "没记过又没有目录要立");
    // 记过的（连同负的答）不借目录也说得出来。
    assert!(lookup(Some(&loaded), true).listing("deepseek").is_some());
    assert_eq!(lookup(Some(&loaded), true).listing("nope"), None);
    assert!(lookup(None, false).listing("deepseek").is_some());
    assert_eq!(lookup(None, false).listing("nope"), None);
    assert!(!flag.load(Ordering::Relaxed), "记过的不立");
}

#[test]
fn a_rebuilt_listing_follows_the_new_catalog() {
    let old = loaded();
    let new = changed();
    let mut recalled = Recalled::new("old", old.source, &old.fetched);
    for ask in listing_asks(&old.catalog) {
        assert!(matches!(
            recalled.listing(Some(&old), &ask, true),
            Recall::Known(_)
        ));
    }
    let rebuilt = recalled.rebuilt(&new, &vendors(), "new");
    let mut differs = false;
    let back = Mutex::new(rebuilt);
    let lookup = CatalogLookup::Recalled(Recalling {
        recalled: &back,
        catalog: None,
        remember: false,
        missed: None,
    });
    for ask in listing_asks(&old.catalog) {
        let expected = listing_side(&new.catalog, &ask);
        differs |= expected != listing_side(&old.catalog, &ask);
        same(&expected, &lookup.listing(&ask), &|| {
            format!("重算的：{ask}")
        });
    }
    assert!(differs, "新旧两份要真有不一样的清单");
    assert_eq!(locked(&back).of(), "new");
}

#[test]
fn merging_keeps_what_is_already_there() {
    let loaded = loaded();
    let vendors = vendors();
    let mut route = Recalled::new("v", loaded.source, &loaded.fetched);
    let _ = route.provider_side(Some(&loaded), "deepseek", None, None, true);
    let mut listing = Recalled::new("v", loaded.source, &loaded.fetched);
    let _ = listing.listing(Some(&loaded), "deepseek", true);
    let _ = listing.model_side(
        Some(&loaded),
        &vendors,
        Some("deepseek"),
        "deepseek-flash",
        None,
        true,
    );
    let _ = listing.provider_side(Some(&loaded), "deepseek", None, None, true);
    let before = route.entries();
    route.merge(listing.clone());
    assert_eq!(route.entries(), before + 2, "重复的那条只算一次");
    assert!(matches!(
        route.listing(None, "deepseek", false),
        Recall::Known(Some(_))
    ));
    assert!(matches!(
        route.model_side(
            None,
            &vendors,
            Some("deepseek"),
            "deepseek-flash",
            None,
            false
        ),
        Recall::Known(_)
    ));
}

#[test]
fn a_recall_without_listings_writes_the_same_text_as_before() {
    let loaded = loaded();
    let mut recalled = Recalled::new("v", loaded.source, &loaded.fetched);
    let _ = recalled.provider_side(Some(&loaded), "deepseek", None, None, true);
    let text = recalled.to_json().expect("写得成");
    assert!(!text.contains("listings"), "路由那份不多这一格：{text}");
    let old = r#"{"v":1,"of":"v","source":"snapshot","fetched":"x","providers":[],"models":[]}"#;
    let read = Recalled::from_json(old).expect("以前写的读得回来");
    assert_eq!(read.entries(), 0);
    assert_eq!(read.to_json().expect("写得成"), old);
}

/// 清单里的每个模型顺手问好（施工 V-32，`fill_listed`）：新目录里多出来的模型不借目录也答得出来，和现算的一样。
#[test]
fn filled_listings_answer_every_listed_model_without_the_catalog() {
    let old = loaded();
    let new = changed();
    let vendors = vendors();
    let mut recalled = Recalled::new("old", old.source, &old.fetched);
    for ask in listing_asks(&old.catalog) {
        let _ = recalled.listing(Some(&old), &ask, true);
    }
    let mut rebuilt = recalled.rebuilt(&new, &vendors, "new");
    rebuilt.fill_listed(&new, &vendors);
    let mut asked = 0;
    for listed in new.catalog.providers() {
        for model in listed.models.keys() {
            let expected = model_side(&new.catalog, &vendors, Some(&listed.id), model, None);
            match rebuilt.model_side(None, &vendors, Some(&listed.id), model, None, false) {
                Recall::Known(side) => same(&expected, &side, &|| format!("{}/{model}", listed.id)),
                Recall::Unknown => panic!("{}/{model} 没问好", listed.id),
            }
            asked += 1;
        }
    }
    assert!(
        new.catalog.model("anthropic", "claude-newest").is_some() && asked > 10,
        "新目录里多出来的模型也在里面"
    );
}
