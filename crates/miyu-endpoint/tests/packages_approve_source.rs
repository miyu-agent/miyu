//! 从软件源装、升级时说清来源和能力（施工 PK-3 上，设计 `61-装包说清来源和能力.md` 第二、四节）：真核心走一遍，假服务器当软件源。
//! 看一眼每个包写 `source`（官方源里官方的、社区收录的）、`feed`、能力，一起装的、推荐的也带，社区的有「注意」；照 `approve` 装上
//! 就批，`held` 只在来路没变时有，本机文件盖过去就没了；升级照来路没变留着批过的，新版不要的去掉、加回来要重新批，看一眼带
//! 来源、能力和一起装的新依赖。

use std::path::Path;

use serde_json::{Value, json};

use miyu_http::testkit::Server;

use crate::support::source::{
    bytes, call_collecting, core, file_entry, keys, serving, signed_entries,
};
use crate::support::*;

/// 打一个扩展包：编号、版本、要的能力、依赖；包文件夹在 `src/<编号>-<版本>/<编号>`。交回文件名、字节，和照它写的索引条目
/// （`official` 照写）。
fn kit(
    home: &Home,
    (id, version): (&str, &str),
    (capabilities, depends): (&[&str], &[&str]),
    official: bool,
) -> ((String, Vec<u8>), Value) {
    let folder = home
        .work
        .join("src")
        .join(format!("{id}-{version}"))
        .join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    let quoted = |all: &[&str]| {
        all.iter()
            .map(|one| format!("{one:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    std::fs::write(
        folder.join("package.toml"),
        format!(
            "[package]\nversion = \"{version}\"\nprotocol = [1, 1]\nname = {{ en = \"{id}\" }}\n\n[depends]\npackages = [{}]\n\n[command]\nname = \"{id}\"\nprogram = \"miyu-no-such-program-anywhere\"\nabout = {{ en = \"K\" }}\n\n[process]\nstart = \"manual\"\ncapabilities = [{}]\n",
            quoted(depends),
            quoted(capabilities)
        ),
    )
    .expect("写得进");
    let name = miyu_source::archive::file_name(id, version, None);
    let out = home.work.join("out").join(&name);
    std::fs::create_dir_all(out.parent().expect("有上一层")).expect("建得了");
    miyu_source::archive::pack(&folder, id, &out).expect("打得成");
    let file = (name, std::fs::read(&out).expect("在"));
    let mut entry = file_entry(id, version, depends, &file);
    entry["official"] = json!(official);
    entry["kind"] = json!("process");
    entry["provides"] = json!({"command": [id]});
    entry["capabilities"] = json!(capabilities);
    (file, entry)
}

fn sign(entries: &[Value], generated: &str, secret: &str) -> (Vec<u8>, Vec<u8>) {
    signed_entries(generated, entries, secret)
}

/// 开关文件里包 `id` 批过的；没记的是 `null`。
fn approved(home: &Home, id: &str) -> Value {
    let text = std::fs::read_to_string(home.root.extensions_file()).unwrap_or_default();
    serde_json::from_str::<Value>(&text).unwrap_or(Value::Null)["approved"][id].clone()
}

/// 能力那一格：编号，`held` 的后面加 `*`。
fn held(listed: &Value) -> Vec<String> {
    listed
        .as_array()
        .into_iter()
        .flatten()
        .map(|one| {
            let id = one["id"].as_str().unwrap_or_default();
            match one["held"] == true {
                true => format!("{id}*"),
                false => id.to_string(),
            }
        })
        .collect()
}

#[tokio::test]
async fn the_preview_says_where_each_package_comes_from_and_holds_only_on_the_same_way() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let (app, mut app_entry) = kit(
        &home,
        ("app", "1.0"),
        (&["tools", "network"], &["lib"]),
        true,
    );
    app_entry["recommends"] = json!(["rec"]);
    let (lib, lib_entry) = kit(&home, ("lib", "1.0"), (&["search"], &[]), false);
    let (_, rec_entry) = kit(&home, ("rec", "1.0"), (&["network"], &[]), false);
    let index = sign(
        &[app_entry, lib_entry, rec_entry],
        "2026-11-05T03:00:00Z",
        &secret,
    );
    let mut replies = serving(&index);
    replies.extend([bytes(&lib.1), bytes(&app.1)]);
    let server = Server::start(replies).await;
    let mirror = format!("{}/m", server.base_url);
    let mut client = Client::connect(core(
        &home,
        "https://unreachable.invalid/",
        &[&mirror],
        trusted,
    ));
    client.hello().await;
    let look = json!({"source": "app", "preview": true});

    let (preview, _) = call_collecting(&mut client, "package.install", look.clone()).await;
    let preview = &preview["result"];
    assert_eq!(
        (&preview["source"], &preview["feed"]),
        (&json!("official"), &json!("official")),
        "{preview}"
    );
    assert_eq!(held(&preview["capabilities"]), ["tools", "network"]);
    let lib_item = &preview["with"][0];
    assert_eq!(
        lib_item["source"], "community",
        "官方源里社区收录的：{preview}"
    );
    assert_eq!(lib_item["feed"], "official");
    assert_eq!(held(&lib_item["capabilities"]), ["search"]);
    let rec_item = &preview["recommends"][0];
    assert_eq!(
        (&rec_item["package"], &rec_item["source"]),
        (&json!("rec"), &json!("community"))
    );
    assert_eq!(held(&rec_item["capabilities"]), ["network"]);
    assert_eq!(
        preview["notice"],
        json!(["lib 是社区包，没有经过官方审核，可能存在风险。"]),
        "几个包的写出是哪几个，不再说沙盒"
    );
    assert_eq!(
        preview["recommends_notice"],
        json!(["rec 是社区包，没有经过官方审核，可能存在风险。"])
    );

    let (reply, _) = call_collecting(
        &mut client,
        "package.install",
        json!({"source": "app", "approve": {"app": ["network"], "lib": ["search"]}}),
    )
    .await;
    assert_eq!(reply["result"]["package"], "app", "{reply}");
    assert_eq!(approved(&home, "app"), json!(["network"]));
    assert_eq!(approved(&home, "lib"), json!(["search"]));
    // 装着的也分得出官方、社区（施工 PK-6）：官方源里官方收录的 app、社区收录的 lib；原来的 `from` 都是 `official`，照旧不动。
    let listed = client.call("p", "package.list", json!({})).await;
    let item = |id: &str| {
        listed["result"]["packages"]
            .as_array()
            .expect("有列表")
            .iter()
            .find(|item| item["package"] == id)
            .cloned()
            .unwrap_or_else(|| panic!("列了 {id}：{listed}"))
    };
    for (id, source) in [("app", "official"), ("lib", "community")] {
        let one = item(id);
        assert_eq!(
            (&one["source"], &one["feed"], &one["from"]),
            (&json!(source), &json!("official"), &json!("official")),
            "{one}"
        );
    }

    // 来路没变：批过的标 `held`，没批的不标。
    let (again, _) = call_collecting(&mut client, "package.install", look.clone()).await;
    assert_eq!(
        held(&again["result"]["capabilities"]),
        ["tools", "network*"],
        "{again}"
    );

    // 本机文件盖过去：来路变了，前面批过的不接着用，再看软件源的那一份也没有 `held`。
    let folder = home.work.join("src/app-1.0/app");
    let local = client
        .call("l", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(local["result"]["package"], "app", "{local}");
    assert_eq!(approved(&home, "app"), Value::Null);
    let info = client
        .call("i", "package.info", json!({"package": "app"}))
        .await;
    assert_eq!(
        info["result"]["source"], "file",
        "本机文件盖过去照这一次重记：{info}"
    );
    assert!(info["result"].get("feed").is_none(), "{info}");
    let (after, _) = call_collecting(&mut client, "package.install", look).await;
    assert_eq!(
        held(&after["result"]["capabilities"]),
        ["tools", "network"],
        "{after}"
    );
    assert!(Path::new(&folder).is_dir());
}

#[tokio::test]
async fn an_upgrade_keeps_what_is_still_wanted_and_asks_again_for_what_comes_back() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let (app1, entry1) = kit(&home, ("app", "1.0"), (&["tools", "network"], &[]), true);
    let (app2, entry2) = kit(&home, ("app", "2.0"), (&["tools"], &["lib"]), true);
    let (lib, lib_entry) = kit(&home, ("lib", "1.0"), (&["search"], &[]), false);
    let (app3, entry3) = kit(
        &home,
        ("app", "3.0"),
        (&["tools", "network"], &["lib"]),
        true,
    );
    let first = sign(&[entry1], "2026-11-05T03:00:00Z", &secret);
    let second = sign(
        &[entry2, lib_entry.clone()],
        "2026-11-06T03:00:00Z",
        &secret,
    );
    let third = sign(&[entry3, lib_entry], "2026-11-07T03:00:00Z", &secret);
    // 先后：索引、签名、app 1.0；新索引、签名，lib、app 2.0；再新的索引、签名，app 3.0。
    let mut replies = serving(&first);
    replies.push(bytes(&app1.1));
    replies.extend(serving(&second));
    replies.extend([bytes(&lib.1), bytes(&app2.1)]);
    replies.extend(serving(&third));
    replies.push(bytes(&app3.1));
    let server = Server::start(replies).await;
    let mirror = format!("{}/m", server.base_url);
    let mut client = Client::connect(core(
        &home,
        "https://unreachable.invalid/",
        &[&mirror],
        trusted,
    ));
    client.hello().await;
    let (reply, _) = call_collecting(
        &mut client,
        "package.install",
        json!({"source": "app", "approve": {"app": ["network", "tools"]}}),
    )
    .await;
    assert_eq!(reply["result"]["package"], "app", "{reply}");
    assert_eq!(approved(&home, "app"), json!(["network", "tools"]));

    client
        .call("s", "package.sync", json!({"force": true}))
        .await;
    let preview = client
        .call("p", "package.upgrade", json!({"preview": true}))
        .await;
    let preview = &preview["result"];
    let row = &preview["upgrades"][0];
    assert_eq!(
        (&row["package"], &row["to"]),
        (&json!("app"), &json!("2.0")),
        "{preview}"
    );
    assert_eq!(
        (&row["source"], &row["feed"]),
        (&json!("official"), &json!("official"))
    );
    assert_eq!(held(&row["capabilities"]), ["tools*"]);
    assert_eq!(
        preview["with"][0]["package"], "lib",
        "一起装的新依赖：{preview}"
    );
    assert_eq!(held(&preview["with"][0]["capabilities"]), ["search"]);
    assert_eq!(
        preview["notice"],
        json!(["lib 是社区包，没有经过官方审核，可能存在风险。"])
    );
    let (upgraded, _) = call_collecting(
        &mut client,
        "package.upgrade",
        json!({"approve": {"lib": ["search"]}}),
    )
    .await;
    assert_eq!(upgraded["result"]["upgraded"][0]["to"], "2.0", "{upgraded}");
    assert_eq!(approved(&home, "app"), json!(["tools"]), "新版不要的去掉");
    assert_eq!(approved(&home, "lib"), json!(["search"]));

    client
        .call("s", "package.sync", json!({"force": true}))
        .await;
    let preview = client
        .call("p", "package.upgrade", json!({"preview": true}))
        .await;
    assert_eq!(
        held(&preview["result"]["upgrades"][0]["capabilities"]),
        ["tools*", "network"],
        "{preview}"
    );
    assert!(preview["result"].get("notice").is_none(), "官方的没有注意");
    let (upgraded, _) = call_collecting(&mut client, "package.upgrade", json!({})).await;
    assert_eq!(upgraded["result"]["upgraded"][0]["to"], "3.0", "{upgraded}");
    assert_eq!(approved(&home, "app"), json!(["tools"]), "加回来的要重新批");
}
