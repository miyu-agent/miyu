//! 登记簿（`docs/designs/26-提示词.md` 第十节，J12）：给模型看的每一份字都登记在那张表里。
//!
//! 门禁查两边一一对上：`resources/` 下每一份文件都在表里，表里每一行都有文件，指纹对得上。指纹是
//! 文件字节的 SHA-256 的前 8 位：字改了就对不上，逼着回登记簿重新量 token、写上为什么改。算在
//! 「文档」那一项里。

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::drawing;

/// 登记簿在仓库里的位置。
pub const PATH: &str = "docs/designs/26-提示词.md";

pub(crate) const SECTION: &str = "登记簿";
pub(crate) const HEADER: [&str; 6] = [
    "文件",
    "进到哪",
    "什么时候加进来",
    "token",
    "为什么加",
    "指纹",
];

/// 给模型看的字都在这个目录里。
pub(crate) const RESOURCES: &str = "resources";

/// 给人看的字放在这样的目录里，不登记（施工 4-5 上）。
const HUMAN: &str = "human";

/// 资源目录最上一层的这个目录放模型资料，是数据，不发给模型，不登记（施工 6-3 上）。
const DATA: &str = "models";

/// 资源目录最上一层的这个目录放软件包（施工 9-1 上，F-8 上起一个文件夹一个包，`packages.md`）：清单是给人看的名字、说明和程序名，
/// 不发给模型，不登记；包带的技能、脚本（施工 X-13 下）是给模型看的，照查（[`CARRIED`]）。
const PACKAGES: &str = "packages";

/// 包文件夹里给模型看的那几个子目录（施工 X-13 下）：技能的 `SKILL.md` 和参考资料，脚本的开头注释；包自己读的数据 `data/`
/// （施工 S-6 上），里面给人看的 `human/` 照旧不查。接入QQ 的包文件夹随施工 S-6 下搬出核心仓库，给它 `data/` 里四份数据的豁免
/// 一起去掉：它的字由它自己的仓库登记（设计 `32-仓库拆分.md` 第九节第 3、6 条）。
const CARRIED: [&str; 3] = ["skills", "scripts", "data"];

/// 资源目录最上一层的这个目录放出厂的预设（施工 P-2 上，`presets.md`）：给人看的名字、说明和开关表，不发给模型，不登记。
const PRESETS: &str = "presets";

/// `software/mermaid/` 整个是数据（字体、三种记号色、源码的上限），不发给模型，不登记（施工 W-4，`mermaid.md`「样子」：
/// 「这条线不加给模型看的字」）。
const SOFTWARE_DIR: &str = "software/";

/// 人格目录里的 `persona.toml` 是给人看的名字、说明和几项配置，不发给模型，不登记（施工 P-1 下，`personas.md`）；同一个目录里
/// `prompts/` 下的照查。
const PERSONAS_DIR: &str = "personas/";
const PERSONA_TOML: &str = "persona.toml";

const DATA_PACKAGES: [&str; 1] = ["mermaid"];

/// `core/view.json` 是数据，不发给模型，不登记：视图投影照它认哪件工具算命令、编辑、子代理、留言（施工 9-8 上，`view.md`）。
/// `core/memory/secrets.toml` 也是：抽取照它认常见的 key 写法、遮掉，不发给模型（施工 R-6 上，`memory.md` 第六条）。
/// `core/memory/recall.toml` 也是：联想照它定每个模型的门槛，不发给模型（施工 R-8，第四条）；`core/memory/search.toml` 是她主动搜
/// 时只有关键词找到的那几条的下限（施工 R-14）。`core/knowledge/recall.toml`
/// 同样：知识库的自动想起照它定门槛（施工 K-6 下，`knowledge.md` 第九条）；`core/knowledge/keys.toml` 是小节的关键词那一行
/// 怎么认（施工 K-9）。`software/net/link_preview.json` 是抓网页的时限、上限、请求头（施工 W-7，`net.md`），`search.json` 是搜索来源的地址、顺序、
/// 冷却（施工 N-2 上）；同一个目录里
/// `web_fetch` 的说明和几句字（施工 N-1）照查。只豁免这几份：`core/` 里别的照查。
const DATA_FILES: [&str; 9] = [
    "core/view.json",
    "core/memory/secrets.toml",
    "core/memory/recall.toml",
    "core/memory/search.toml",
    "core/knowledge/recall.toml",
    "core/knowledge/search.toml",
    "core/knowledge/keys.toml",
    "software/net/link_preview.json",
    "software/net/search.json",
];

/// 查一遍，交回对不上的地方。
pub fn check(root: &Path) -> Vec<String> {
    let text = match std::fs::read_to_string(root.join(PATH)) {
        Ok(text) => text,
        Err(e) => return vec![format!("读不了登记簿 {PATH}：{e}")],
    };
    let rows = match rows(&text) {
        Ok(rows) => rows,
        Err(e) => return vec![format!("登记簿 {PATH} 读不出来：{e}")],
    };
    let mut files = BTreeMap::new();
    match walk(&root.join(RESOURCES), "", &mut files) {
        Ok(()) => compare(&rows, &files),
        Err(e) => vec![e],
    }
}

/// 表里的每一行：文件（`resources/` 下的路径）到登记的指纹。
fn rows(text: &str) -> Result<BTreeMap<String, String>, String> {
    let section = drawing::section(text, SECTION)?;
    let tables = drawing::tables(&section);
    let table = drawing::find_table(&tables, &HEADER)?;
    let mut rows = BTreeMap::new();
    for row in &table.rows {
        let quoted = |cell: Option<&String>| {
            cell.and_then(|cell| drawing::backticked(cell).into_iter().next())
                .ok_or_else(|| format!("这一行的文件、指纹要写在反引号里：{}", row.join(" | ")))
        };
        let file = quoted(row.first())?;
        let fingerprint = quoted(row.last())?;
        if rows.insert(file.clone(), fingerprint).is_some() {
            return Err(format!("{file} 登记了两行"));
        }
    }
    Ok(rows)
}

/// `dir` 下每一份文件：相对 `resources/` 的路径（用 `/` 分开，三个平台一样）到字节。
fn walk(dir: &Path, prefix: &str, files: &mut BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    let unreadable = |e: std::io::Error| format!("读不了 {}：{e}", dir.display());
    for entry in std::fs::read_dir(dir).map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        if entry.file_type().map_err(unreadable)?.is_dir() {
            // 给人看的字、模型资料、mermaid 的 style.json、net 的 link_preview.json 都不发给模型，不进登记簿（26 第十节，施工
            // 4-5 上、6-3 上、W-4、W-7）。QQ 桥原来 WebUI 的页面 `software/onebot/web/` 随施工 O-28 下去掉，网页软件的 `web/` 随施工
            // S-5 搬出核心仓库，豁免都一起去掉（施工 T-13）；接入QQ 的包文件夹随施工 S-6 下搬走，给它数据的豁免也去掉。
            if prefix.is_empty() && name == PACKAGES {
                carried(&entry.path(), &format!("{path}/"), files)?;
                continue;
            }
            if name == HUMAN
                || (prefix.is_empty() && (name == DATA || name == PRESETS))
                || (prefix == SOFTWARE_DIR && DATA_PACKAGES.contains(&name.as_str()))
            {
                continue;
            }
            walk(&entry.path(), &format!("{path}/"), files)?;
        } else if !(name == PERSONA_TOML
            && prefix.starts_with(PERSONAS_DIR)
            && prefix.matches('/').count() == 2)
            && !DATA_FILES.contains(&path.as_str())
        {
            files.insert(path, std::fs::read(entry.path()).map_err(unreadable)?);
        }
    }
    Ok(())
}

/// 出厂的软件包 `dir`（前缀 `prefix`）：每个包只查带的技能、脚本（[`CARRIED`]），清单这些不查。
fn carried(dir: &Path, prefix: &str, files: &mut BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    let unreadable = |e: std::io::Error| format!("读不了 {}：{e}", dir.display());
    for entry in std::fs::read_dir(dir).map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        if !entry.file_type().map_err(unreadable)?.is_dir() {
            continue;
        }
        let package = entry.file_name().to_string_lossy().into_owned();
        for sub in CARRIED {
            let inner = entry.path().join(sub);
            if inner.is_dir() {
                walk(&inner, &format!("{prefix}{package}/{sub}/"), files)?;
            }
        }
    }
    Ok(())
}

/// 两边比：没登记的、登记了却没有文件的、指纹对不上的。
fn compare(rows: &BTreeMap<String, String>, files: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    let mut problems = Vec::new();
    for (file, bytes) in files {
        let actual = fingerprint(bytes);
        match rows.get(file) {
            None => problems.push(format!(
                "resources/{file} 没登记：给模型看的字都要登记在 {PATH} 第十节（进到哪、什么时候加进来、多少 token、为什么加），指纹是 {actual}"
            )),
            Some(recorded) if *recorded != actual => problems.push(format!(
                "resources/{file} 的字变了，登记簿里的指纹还是 {recorded}：重新量 token、写上为什么改，指纹换成 {actual}"
            )),
            Some(_) => {}
        }
    }
    for file in rows.keys().filter(|file| !files.contains_key(*file)) {
        problems.push(format!("登记簿里有 {file}，resources/ 下却没有这份文件"));
    }
    problems
}

/// 指纹：字节的 SHA-256 的前 8 位十六进制。
fn fingerprint(bytes: &[u8]) -> String {
    Sha256::digest(bytes)[..4]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "\
### 十、登记簿

| 文件 | 进到哪 | 什么时候加进来 | token | 为什么加 | 指纹 |
|---|---|---|---|---|---|
| `core/a.txt` | 事实 | 断了的那一次 | 3 | 施工 3-5 | `2cf24dba` |
| `core/b.txt` | 事实 | 每次 | 1 | 施工 3-5 | `00000000` |

### 附录
";

    fn files(entries: &[(&str, &str)]) -> BTreeMap<String, Vec<u8>> {
        entries
            .iter()
            .map(|(path, text)| ((*path).to_string(), text.as_bytes().to_vec()))
            .collect()
    }

    #[test]
    fn the_fingerprint_is_the_first_eight_hex_of_the_sha256() {
        assert_eq!(fingerprint(b"hello"), "2cf24dba");
    }

    #[test]
    fn the_rows_are_read_from_the_section_table() {
        let recorded = rows(TABLE).unwrap();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded["core/a.txt"], "2cf24dba");
        assert!(rows("### 十、别的\n").is_err());
        let twice = TABLE.replace("`core/b.txt`", "`core/a.txt`");
        assert!(rows(&twice).unwrap_err().contains("登记了两行"));
    }

    #[test]
    fn every_mismatch_is_named() {
        let recorded = rows(TABLE).unwrap();
        // a 对上了；b 的字变了；c 没登记。
        let problems = compare(
            &recorded,
            &files(&[
                ("core/a.txt", "hello"),
                ("core/b.txt", "x"),
                ("core/c.txt", "y"),
            ]),
        );
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("core/b.txt 的字变了"));
        assert!(problems[1].contains("core/c.txt 没登记"));
        // 登记了却没有文件。
        let problems = compare(&recorded, &files(&[("core/a.txt", "hello")]));
        assert_eq!(
            problems,
            ["登记簿里有 core/b.txt，resources/ 下却没有这份文件"]
        );
    }

    #[test]
    fn human_folders_are_not_walked() {
        let dir = std::env::temp_dir().join(format!("miyu-ledger-{}", std::process::id()));
        for (path, text) in [
            ("core/a.txt", "a"),
            ("core/view.json", "{}"),
            ("core/x/view.json", "{}"),
            ("core/human/zh.json", "{}"),
            ("software/x/human/en.json", "{}"),
            ("software/x/tools/t.json", "{}"),
            ("models/models-dev.json", "{}"),
            ("web/web.json", "{}"),
            ("web/pages/index.html", "x"),
            ("core/web/w.txt", "w"),
            ("core/models/m.txt", "m"),
            ("software/mermaid/style.json", "{}"),
            ("software/net/link_preview.json", "{}"),
            ("software/x/mermaid/not_special_here.json", "{}"),
            ("software/x/net/not_special_here.json", "{}"),
            ("personas/x/persona.toml", "[persona]"),
            ("personas/x/prompts/persona.md", "p"),
            ("personas/x/prompts/persona.toml", "q"),
            ("packages/web/package.toml", "[package]"),
            ("packages/kit/skills/notes/SKILL.md", "---"),
            ("packages/kit/skills/notes/reference.md", "r"),
            ("packages/kit/scripts/dice.py", "#!"),
            ("packages/kit/page/index.html", "x"),
            ("core/packages/p.txt", "p"),
            ("presets/dev.toml", "[preset]"),
            ("core/presets/p.txt", "p"),
            ("packages/kit/data/notes.txt", "n"),
            ("packages/kit/data/defaults.toml", "[chatty]"),
            ("packages/kit/data/bridge.json", "{}"),
            ("packages/kit/data/human/en.json", "{}"),
        ] {
            let path = dir.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let mut found = BTreeMap::new();
        let walked = walk(&dir, "", &mut found);
        std::fs::remove_dir_all(&dir).unwrap();
        walked.unwrap();
        // 给人看的字、最上一层的模型资料、软件包清单和出厂的预设、software/mermaid/、software/net/、人格目录的 persona.toml、
        // 视图投影的工具分类 core/view.json 不登记，别的照查（别处叫 models、web、mermaid、net 的目录、别的包的 web/ 照查：只有正好
        // 这几处才豁免；最上一层的 web/ 随施工 T-13 去掉豁免，再放进来照查）。软件包里带的技能、脚本、数据照查，数据里的出厂参数、
        // 桥的数这类文件名不豁免（接入QQ 那四份的豁免随施工 S-6 下去掉），数据里给人看的字、清单、后台页这些不查（施工 X-13 下、
        // S-6 上）。
        assert_eq!(
            found.keys().collect::<Vec<_>>(),
            [
                "core/a.txt",
                "core/models/m.txt",
                "core/packages/p.txt",
                "core/presets/p.txt",
                "core/web/w.txt",
                "core/x/view.json",
                "packages/kit/data/bridge.json",
                "packages/kit/data/defaults.toml",
                "packages/kit/data/notes.txt",
                "packages/kit/scripts/dice.py",
                "packages/kit/skills/notes/SKILL.md",
                "packages/kit/skills/notes/reference.md",
                "personas/x/prompts/persona.md",
                "personas/x/prompts/persona.toml",
                "software/x/mermaid/not_special_here.json",
                "software/x/net/not_special_here.json",
                "software/x/tools/t.json",
                "web/pages/index.html",
                "web/web.json",
            ]
        );
    }
}
