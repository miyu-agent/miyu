//! 软件包带来的知识库（施工 K-8，`docs/blueprint/knowledge.md`「对外的样子」包带来的那一节）：真核心装上一个带 `kb/world/`
//! 的包，`kb.list` 当场有它、带 `package`，搜得到；改名字、说明、删拒 `kb_packaged`，只改自动想起的照改、记在账号自己的
//! `kb.toml`；要建同编号的拒 `kb_exists`。卸掉以后列表里没了，索引、开关一起清掉。核心起来时照核对过的清单：没收的包不带库。

use std::path::PathBuf;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 工作目录里放一个只带知识库的包 `world-pack`：`kb/world/` 里 `kb.toml` 和一篇人物。交回包的文件夹。
fn world_pack(home: &Home) -> PathBuf {
    let folder = home.work.join("world-pack");
    let base = folder.join("kb/world");
    std::fs::create_dir_all(&base).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        "[package]\nname = { en = \"World\", zh = \"雾港世界观\" }\n",
    )
    .expect("写得进");
    std::fs::write(base.join("kb.toml"), "name = \"雾港\"\nauto = true\n").expect("写得进");
    std::fs::write(base.join("people.md"), "# 人物\n\n## 艾琳\n\n守灯人。\n").expect("写得进");
    folder
}

async fn bases(client: &mut Client) -> Vec<Value> {
    let reply = client.call("l", "kb.list", json!({})).await;
    reply["result"]["bases"].as_array().expect("有").clone()
}

#[tokio::test]
async fn a_package_brings_a_read_only_base_whose_switch_is_yours() {
    let home = Home::new();
    let folder = world_pack(&home);
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    assert!(bases(&mut client).await.is_empty());

    let reply = client
        .call("i1", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(reply["result"]["package"], "world-pack", "{reply}");
    let listed = bases(&mut client).await;
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0]["id"], "world");
    assert_eq!(listed[0]["name"], "雾港");
    assert_eq!(listed[0]["package"], "world-pack");
    assert_eq!(listed[0]["auto"], true);
    let found = client
        .call("s1", "kb.search", json!({"query": "守灯人"}))
        .await;
    assert_eq!(found["result"]["hits"][0]["kb"], "world", "{found}");

    let renamed = client
        .call("u1", "kb.update", json!({"kb": "world", "name": "新名字"}))
        .await;
    assert_eq!(reason(&renamed), Some("kb_packaged"), "{renamed}");
    let deleted = client.call("d1", "kb.delete", json!({"kb": "world"})).await;
    assert_eq!(reason(&deleted), Some("kb_packaged"), "{deleted}");
    let made = client.call("c1", "kb.create", json!({"kb": "world"})).await;
    assert_eq!(reason(&made), Some("kb_exists"), "{made}");
    let off = client
        .call("u2", "kb.update", json!({"kb": "world", "auto": false}))
        .await;
    assert_eq!(off["result"]["auto"], false, "{off}");
    assert_eq!(off["result"]["package"], "world-pack");
    let mine = home.root.path().join("kb.toml");
    assert!(
        std::fs::read_to_string(&mine)
            .expect("记下了")
            .contains("\"world-pack/world\" = false")
    );

    let removed = client
        .call("r1", "package.remove", json!({"package": "world-pack"}))
        .await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    assert!(bases(&mut client).await.is_empty(), "卸掉以后没了");
    assert!(
        !home
            .root
            .path()
            .join("index/kb/packages/world-pack")
            .exists(),
        "索引跟着删"
    );
    assert!(
        !std::fs::read_to_string(&mine)
            .unwrap_or_default()
            .contains("world-pack"),
        "开关跟着删"
    );
}

#[tokio::test]
async fn the_core_starts_with_the_packages_it_accepted() {
    // 核心起来时照核对过的清单换上（`Core::with_packages`）：装好了的包带的库当场在；和核心模块同名、写了配置项的包清单
    // 不收（`settings_taken`），它带的库也不在。
    let home = Home::new();
    home.write(
        "packages/world-pack/package.toml",
        "[package]\nname = { en = \"World\" }\n",
    );
    home.write(
        "packages/world-pack/kb/world/people.md",
        "# 人物\n\n## 艾琳\n\n守灯人。\n",
    );
    home.write(
        "packages/ui/package.toml",
        "[package]\nprotocol = [1, 1]\nname = { en = \"Ui\" }\n\n[settings.port]\ntype = \"int\"\ndefault = 1\nlayers = [\"system\"]\nname = { en = \"Port\" }\n\n[ui]\n",
    );
    home.write("packages/ui/kb/rules/a.md", "# 规矩\n");
    let mut client = Client::connect(packaged::core(&home));
    client.hello().await;
    let listed = bases(&mut client).await;
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(
        (listed[0]["id"].clone(), listed[0]["package"].clone()),
        (json!("world"), json!("world-pack"))
    );
}

/// 把 `from` 整个拷到 `to`。
fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("建得了目录");
    for entry in std::fs::read_dir(from).expect("读得了目录") {
        let entry = entry.expect("读得了目录项");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("认得出种类").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("拷得了");
        }
    }
}

#[tokio::test]
async fn a_shipped_package_base_wins_over_a_home_one_whatever_the_ids() {
    // 施工 F-8 六补（设计 31 定了的 J）：核心升级后出厂的包新带了和装着的包同编号的库，出厂的赢，不照包的编号。
    let home = Home::new();
    let resources = home.work.join("resources");
    copy_tree(&default_resources(), &resources);
    let shipped = resources.join("packages/z-lore");
    std::fs::create_dir_all(shipped.join("kb/lore")).expect("建得了");
    std::fs::write(
        shipped.join("package.toml"),
        "[package]\nname = { en = \"Lore\" }\n",
    )
    .expect("写得进");
    std::fs::write(shipped.join("kb/lore/a.md"), "# 出厂的\n").expect("写得进");
    home.write(
        "packages/a-lore/package.toml",
        "[package]\nname = { en = \"Mine\" }\n",
    );
    home.write("packages/a-lore/kb/lore/b.md", "# 装的\n");
    let mut client = Client::connect(home.core_with_resources(&Script::new([]), resources));
    client.hello().await;
    let listed = bases(&mut client).await;
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(
        (listed[0]["id"].clone(), listed[0]["package"].clone()),
        (json!("lore"), json!("z-lore"))
    );
}
