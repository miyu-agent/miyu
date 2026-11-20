## 施工单 FX-7：钉旧核心的程序读得了新字表，升级不碰系统装的包、跳过要更新 Miyu 的

状态：做完了，等合并（分支 `step/FX-7`，从 main 开；主会话 2026-11-20 派工）。蓝图 `store/resources.md`「怎么走」第 3 条、「样子」「守着它的」，`client.md`「门面上有什么」的 `texts`、「规矩」第 4 条，`packages.md`「系统加装」「格式」第 12 条，`protocol.md`「`package.upgrades`、`package.upgrade`」，`cli/pkg.md` 的 `upgrade` 和字表。

### 目的

2026-11-20 把核心 rc2.4 装到项目主人本机时撞上三处：

1. **接入通讯平台起不来**：报 `/usr/lib/miyu/resources/core/human/zh.json: unknown field option_notices`。它钉着旧的核心 crate，读的是新核心装好的字表；施工 DSR-2 给一项的字加了 `option_notices`，旧的读法带 `deny_unknown_fields`，整份拒掉。以后核心每给字表加一格，钉旧核心的扩展、头都会撞一次。
2. **`miyu pkg upgrade` 整次失败**：本机的终端、网页是 pacman 装的（`pkg list` 的 `source` 是 `distro`）。盘点只看本地库有没有来源仓库的记录，不看认的那一份在哪一层：本地库里留着同编号的旧记录时，系统装的包也被当成能升的；索引里的新版要 Miyu 2026.11.0，装的时候报「这个包要 Miyu 2026.11.0 以上，先更新 Miyu」，整个请求失败，别的包也没升。
3. **`Human::with_dir` 被放到了 `testkit` 后面**：施工 AU-P1d 判「只有测试用」时只查了核心仓库，接入通讯平台的正式代码（`texts.rs`、`rules/names.rs`）在用它，只好临时打开 `testkit`。

### 改成什么样

- **字表不认识的格不管**：`human/<语言>.json` 的最外层（`File`）、工具的样子（`Face`）、`config` 那一格（`ConfigWords`）、一项的字（`ItemWords`）去掉 `deny_unknown_fields`。别的带 `deny_unknown_fields` 的结构逐个看过，只有这四个是「钉旧核心的程序读新核心装的资源」：协议参数、包清单 `package.toml`、配置文件、知识库 `kb.toml`、向量模型的清单、工具说明（接入通讯平台只读自己包里的）、`core/view.json`（只有核心读）照旧严格，软件源索引本来就不严格。
- **出厂的字表写错了格照样红**：`miyu-store` 的单元测试 `human/tests.rs` 把每一份出厂的字表（资源目录的 `core/`、`software/*/`、`packages/*/data/`，每种语言）读一遍、照读到的样子写回 JSON，原文里写回去没有的格就是这一版不认识的，报出路径。为此 `ConfigWords`、`ItemWords` 多派生 `Serialize`，`File` 只在测试里派生。另一条测试证明这个检查拦得住拼错的格。
- **升级的盘点只看家目录这一层**：`package.upgrades` 只看认的那一份在 `home` 的包；出厂的、系统加装的不看，本地库里留着旧记录也不看。照 pacman：别的包管理器装的不归它升级。
- **新版要更新的 Miyu 的不算能升**：软件源索引的每个包多一格可选的 `miyu`（清单 `[package] miyu` 的原样，`miyu-index` 收录时写）；盘点照它和正在跑的这一版比，比不过的进新的一格 `needs_newer_miyu: [{"package", "to", "miyu"}]`，不进 `upgrades`，所以 `package.upgrade` 不升它、别的照升。命令行在标准错误上警告一句「tui 2026.11.0 需要 Miyu 2026.11.0，跳过」，退出码 0。比法同读清单时的 `needs_newer_miyu`（`miyu_config::package::needs_newer`，开发版 `0.0.0` 都给得了）。
- **恢复成正式接口**：`Human::with_dir`（接入通讯平台正式在用）、`Packaged::as_released`（终端的测试在用）去掉 `testkit` 的门槛。施工 AU-P1d 放到 `testkit` 后面的另外三个（`builtin_tools`、`Watcher::by_helper`、`Personas::at_home`）在 `miyu-agent` 目录下的各仓库里查过，没有下游在用，不动。

### 不做什么

- 不改「升级一个包失败就整次失败」的一般规矩：要更新 Miyu 的在盘点时就分出去了，下载失败这类真出错照旧报错。
- 软件源要用这一版以后的 `miyu-index` 重新生成索引才有 `miyu` 这一格；没有时照旧算能升，装的时候照旧报 `needs_newer_miyu`。重新生成索引是打包仓库的事，不在这一步。
- 工具样子的 `block` 写了不认识的值仍然整份读不成（这是值，不是格），等真加第三种时再定。

### 验收

- 改之前是红的（把 `ItemWords` 的 `deny_unknown_fields` 加回去、盘点去掉层和 Miyu 版本的判断，跑同样的测试）：
  - `crates/miyu-store/tests/human.rs` 的 `fields_this_version_does_not_know_are_skipped` 报 `unknown field option_hints, expected one of name, description, options, option_descriptions, option_notices, fields, suggestions`，和项目主人本机接入通讯平台报的是同一句；
  - `crates/miyu-endpoint/src/packages/source/upgrade/tests.rs` 的 `distro_packages_are_left_alone_and_a_newer_miyu_is_skipped_not_failed` 断言失败（系统加装的、出厂的进了 `upgrades`，要更新 Miyu 的也进了）；
  - `crates/miyu-store/src/human/tests.rs` 的 `a_misspelled_field_is_found_at_every_level` 在严格的读法下直接读不成。
- 改之后：上面几条通过；`shipped_words_have_no_fields_this_version_does_not_know` 查过全部出厂字表；`miyu-config` 的 `the_index_can_say_a_package_needs_a_newer_miyu`、`miyu-source` 的 `collect/tests/carried.rs`、`miyu-cli` 的 `a_package_needing_a_newer_miyu_is_said_and_skipped` 通过。
- 本机门禁 12 项全过，三平台 CI 全绿。

### 怎么实测验收

2026-11-21 在沙箱里用这一支编出来的 `miyu`、`miyu-index` 走了一遍（`MIYU_HOME`、`HOME`、`XDG_DATA_DIRS` 都指向临时目录；脚本见施工时的草稿，没有入库）：

1. 用 `miyu pkg pack` 打 `alpha`、`gamma`、`beta` 三个只带一个技能的包，`miyu-index folder` 收成索引、`miyu-index sign` 签名，`python3 -m http.server` 在本机端口上发布；配置 `packages.sources.local` 指向它。
2. 从这个源装 `alpha 1.0.0`、`gamma 1.0.0`、`beta 1.0.0`。把 `gamma` 的包目录从数据根的 `opt/` 挪进 `XDG_DATA_DIRS/miyu/packages/`，本地库里的记录留着：`miyu pkg info gamma` 的来源一行写「系统的包管理器安装」，和项目主人本机的终端、网页一样。
3. 源里换成 `alpha 2.0.0`、`beta 2.0.0`（清单写 `miyu = "2099.1.0"`，索引里带上了 `"miyu": "2099.1.0"`）、`gamma 2.0.0`，运行 `miyu pkg update`、`miyu pkg upgrade --yes`：
   - 标准错误上一行 `miyu: 警告：beta 2.0.0 需要 Miyu 2099.1.0，跳过`；
   - `已升级 alpha 1.0.0 → 2.0.0`；
   - `gamma` 没有升，仍然是 1.0.0；
   - 退出码 0。

字表那一半没有单独起扩展实测：接入通讯平台读字表走的就是 `Human::load`、`Human::with_dir`，集成测试用一份带新格的字表喂给这两个函数，改之前报的就是主人本机那一句。

自己验收时可以运行：

```bash
cargo test -p miyu-store --test all human::
cargo test -p miyu-store --lib human::
cargo test -p miyu-endpoint --lib packages::source::upgrade
cargo test -p miyu-cli --lib pkg::upgrade
```
