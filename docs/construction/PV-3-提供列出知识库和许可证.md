## PV-3 提供列出知识库，清单可以写许可证

状态：施工完，待审（2026-11-12）。接 AD-1 第四节：A「提供不列知识库」（技术细节，按推荐做）、产品取舍 1「社区包要不要写许可证」（项目主人
2026-11-12 定：加一格、可以不写）。

### 目的

- 装之前看一眼、「已安装」、软件源里都看得出一个包带了知识库。
- 作者能在清单里写许可证，看包的人（`miyu pkg info`、头的软件包信息页、软件市场）知道能怎么用。

### 蓝图改哪几节

- `packages.md`：
  - 「格式」的例子清单多 `license` 一行；
  - 「对外的样子」`package.list` 表的 `provides` 种类多 `knowledge` 和它的认法；
  - 「依赖」末尾加第 9 条「软件源索引里的 `license`」；
  - 「本地库」第 4 条 `package.info` 多许可证；
  - 「出错」多 `bad_license`；
  - 「守着它的」`packages_provides.rs` 那一行补知识库、许可证，另加一行 `credits/tests.rs`、`collect/tests/carried.rs`、
    `packages_info.rs`、`packages_search.rs`。
- `protocol.md`：`package.info` 装着的、没装的多 `license`，装着的 `contains` 多 `knowledge`；`package.search` 每一项多 `license?`。
- `knowledge.md`「软件包带来的」多一条：包带了哪几个库列在 `provides`、`contains` 的 `knowledge`。
- `cli/pkg.md`：装前「包含」多「知识库 <编号>」；`info` 在「上游」后面多「许可证」，键表、「带的」字表跟着加。
- 参考 `packages`（中英）：`package.list` 的 `provides` 种类、`package.info` 三种回应、`install` 预览带的几格、`package.search` 每一行。
- 作者文档 `manifest`（中英）：例子清单和 `[package]` 表多 `license`。README「发到软件源」没提 LICENSE 文件，不动。

### 怎么做

- `miyu_source::provides::of` 多 `knowledge`：`kb/` 下的目录（`symlink_metadata`，不跟链接，同 `miyu_store::knowledge::carried`
  用的 `DirEntry::file_type`），编号照包编号的写法（`index::plain_id`，同 `miyu_store` 的 `valid`；这个 crate 不依赖
  `miyu-store`，照写一份）。`kb.toml` 不要求：核心认包里的库本来就不看它（K-8、`knowledge/place.rs` 的 `carried_in`），只看
  目录，`provides` 要和核心真认的一样。
- 端点 `preview.rs` 的 `Content` 多 `knowledge`（`miyu_store::knowledge::carried`），看一眼、`package.info` 装着的和带 `path`
  的 `contains` 都从它来。
- `[package] license`：和作者、上游一起在 `miyu-config` 的 `credits.rs` 读，作者、许可证共用「一行字」那一段；`fields.rs` 的
  `PACKAGE` 加 `license`；新代码 `bad_license`，三种语言各一句。
- 带到：`package.info` 装着的（`info.rs`，不放进 `local::info`：她的软件包工具用那一份）、带 `path` 的（`file_info.rs`）、软件源的
  （`source/look.rs`）；软件源索引 `Entry.license`（收录照清单抄）；`package.search` 每一行（`source/search.rs`）。
- `miyu pkg info`：`InfoKey::License`（「许可证」/`License`），排在「上游」后面；装前「包含」、`info` 的「提供」多
  `Carried::Knowledge`（「知识库 <编号>」/`knowledge base <id>`）。

### 定了的

- 许可证只做最小的校验：是字、去掉前后空白 1 到 100 个字、没有控制字符（换行也算），不查是不是合法的 SPDX，文档建议写 SPDX
  （项目主人 2026-11-12 定加一格、可以不写；校验的细节按推荐定）。
- `package.list` 不带许可证（这一步只要信息页、市场看得到）；她的软件包工具不带（不多给模型字）。

### 不做什么

- 不改终端、网页：显示许可证、知识库是各自的会话的事，协议参考写清楚了。
- 不改软件源仓库：索引照旧 `version` 1，旧的客户端照略过新格子；软件源仓库换 `miyu-rev` 以后收录才写这两格，由管软件源的一步做。
- 不给作者文档 README 加许可证的说明（「发到软件源」一节没提 LICENSE 文件）。

### 验收

- `cargo test -p miyu-config package::credits`：许可证写了的读出来（去掉前后空白，不是 SPDX 的照收，100 个字照收）、没写的没有；
  空的、全是空白的、不是字的、数组、两行的、101 个字的报 `bad_license`，`detail` 是 `package.license`，报在写的那一行。
- `cargo test -p miyu-source collect::tests::carried`：收录的条目 `provides.knowledge` 是 `["atlas", "lighthouse"]`（没有
  `kb.toml` 的 `atlas` 也算；`Bad Name`、`9lives` 两个写法不对的目录、`kb/readme.md` 不算），`license` 照清单去掉空白；没带库、
  没写许可证的包没有这两格，写出的 JSON 里没有 `license`，读得回去。
- `cargo test -p miyu-endpoint --test all packages_provides packages_info packages_search`：带 `path` 看一眼、装上以后的
  `package.list`、`package.info` 都列 `knowledge`、带 `license`；没写的没有；软件源的 `package.info`、`package.search` 照索引带
  `license`，没有的不写。
- `cargo test -p miyu-cli pkg::info`：「提供」印「知识库 atlas、知识库 lighthouse」，写了许可证的在「上游」下面印「许可证 : …」，
  没写的不印。
- 改前红：不改 `provides.rs`、`collect.rs`、`preview.rs`、`info.rs`、`file_info.rs`、`look.rs`、`search.rs`、CLI 的两处时，上面
  这几条测试红。

### 风险

- `package.list` 每一项多读一次 `kb/` 目录：同 PV-2，几十个包各一次 `read_dir`，不另做缓存。
- 软件源换 `miyu-rev` 以前，下载软件里看不到知识库、许可证；装着的、本机文件的照本机现算，不受影响。
