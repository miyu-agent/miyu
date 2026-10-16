## 施工单 S-6 上：接入QQ 只依赖门面、从包文件夹读自己的文件

状态：待施工。设计 `32-仓库拆分.md` 第九节；项目主人 2026-10-16 定接入QQ 拆出核心仓库、单独发、自己装。接入QQ 的会话 10-16 回话：没有在途的活，随时可以拆。

### 起因

拆出去以后桥照 git 依赖钉核心，要和两个界面一样只借门面 `miyu-client`（第二节第 4 条：先在一个仓库里收紧）。自己装的包住在家目录的 `packages/onebot/`，资源目录里没有 `software/onebot/`，桥得能找到自己的包文件夹。

### 这一步做什么

1. **`MIYU_PACKAGE_DIR`**：核心拉起扩展（`extensions/supervise.rs`）、命令行转交包的命令（`miyu-cli` 的 `packages::forward`，Windows 那一份同样）时多设这一个：这个包的文件夹（出厂的在资源目录、自己装的在家目录，哪一层就是哪一层）。`MIYU_HOME`、`MIYU_RESOURCES` 照旧。作者说明第四节第 1 条、`extensions.md` 第 122 行那一条一起改。
2. **包文件夹带数据**：`resources/software/onebot/` 整个挪进 `resources/packages/onebot/data/`（`git mv`）。读的地方全部改成相对包文件夹的 `data/`：照 `MIYU_PACKAGE_DIR` 读，没设的（老核心、手跑）退回资源目录的 `packages/onebot/data/`。逐处（接入QQ 的会话 10-16 审出来的）：
   - `tuning`（`FILE` 带的 `software/onebot/` 前缀）、`rules`（第 84 行自己拼的路径）、`settings`（读自己清单的出厂值，照包文件夹的 `package.toml`）；
   - 平台工具的 `Factory::load`（`main.rs`、`venue.rs`）：`tools/`、`facts/`、`tool-results/`、`judge/`；
   - **给人看的字**：`Human::load` 只扫资源目录的 `software/*/human/`，挪走以后桥的字会静静地没了（读不到不算错）。`Human` 加一个读法：从一个文件夹补读一份、编号前缀照给的（`software/onebot`），核心那份照旧从资源目录读；门面的 `texts` 组转出它。
   - 测试里写死源码路径的跟着改：`miyu-chat` 的 `include_str!`（`moderation`、`rules/resolve` 的 `tests.rs`，`params`、`judge` 的 `test_support.rs`），`miyu-onebot` 的 `decide`、`applied`、`outbound`、`ask`、`provider` 的 `tests.rs`；`tests/support/spawning.rs` 直接拉桥时也设 `MIYU_PACKAGE_DIR`，集成测试走正路、不走回退。
   - 登记簿门禁的四份豁免、设计 26 第十节里的路径照新位置改。
3. **门面加组**：照桥和群聊内核实际用到的（2026-10-16 数的）转出：
   - `ids`：`ExternalId`、`VenueId`、`Seq`、`ContentHash`；`time`：`Timestamp`、`UtcOffset`；
   - `events`：`Event`、`Body`、`By`、`FormatError`；
   - `tooltext`：`miyu_tool::load`（`spec`、`text`、`LoadError`）、`Spec`；
   - `settings`：`Kind`、`Secret`、`parse::{read, why}`、`problem::{At, Code, Severity, got, nearest, tell}`；
   - `http`：`get`、`fetcher`、`Get`、`Got`、`Proxy`、`is_loopback_url`；
   - `places` 补 `generated::write`、`config_file`、`packages::{Packages, Issue}`；`texts` 补 `HumanError`。
   门面测试照旧每一样用一遍。
4. **桥、群聊内核只依赖门面**：两个 `Cargo.toml` 的正式依赖只剩 `miyu-client`（加各自的外部 crate）；开发依赖照旧（设计 32 第四节第 2 条）。分层门禁照旧。

### 定了的（主会话，技术题照推荐）

- 包文件夹的路径用环境变量交，不让扩展自己拼 `MIYU_HOME/home/admin/packages/<编号>`：家目录的布局是核心的事，出厂、自己装两层由核心挑。
- 数据放 `data/` 不放包文件夹根：根上是清单和 `page/`、`bin/`，作者说明里各有各的位置。

### 施工时定的

- 门面多转出两样施工单没数到的：`events` 里的 `Block`、`tooltext` 里的 `Access`。桥用到它们（拼内容块、桥的工具的访问类别），只依赖门面就得有（2026-10-16 施工时，主会话认可）。
- `miyu_tool::load` 加 `spec_in`、`text_in`：原来的 `spec`、`text` 只认资源目录的 `software/<包>/`，读不到包文件夹的 `data/`；新的两个给的是那个目录本身，原来的两个改成调它们，别的调用方不动（2026-10-16 施工时，主会话认可）。
- 命令行拉起整页界面（`head.rs`）不设 `MIYU_PACKAGE_DIR`：施工单只要转交包的命令设，界面包现在不读自己包里的文件（2026-10-16 施工时，主会话认可）。
- 桥的工具给人看的名字：核心不再读桥的 `human/` 以后，桥照第三方包的路子在 `provide` 里每件带 `display_name`（`{zh, en, ja}`，照包文件夹 `data/human/<语言>.json` 的 `tools.<名字>.name`），核心照它进 `human.get`。名字后面跟哪一个参数照原来的 `tools.<名字>.subject`（`skip_reply` 的 `reason`、`mute` 的 `seconds`、`fetch_media` 的 `msg`）：`provide` 的每件工具多一格可选的 `subject`，要是 `input_schema` 的 `properties` 里的名字，不是的 `bad_tool`（`problem` 是 `subject`）；`human.get` 里扩展登记的工具写了的带 `{"name", "subject"}`，没写的照旧只有 `name`；`tool.result` 记下的 `display_name` 不动（2026-10-16 主会话定）。

### 留给 S-6 中

- 群聊内核 `miyu-chat` 照旧直接依赖 `miyu-kernel`、`miyu-config`，不改成只依赖门面：它在第 2 层（纯逻辑），门面 `miyu-client` 在第 3 层，第 2 层依赖第 3 层分层门禁不过。拆仓时这两个和门面一起照 git 依赖钉核心的同一个提交（2026-10-16 主会话定）。
- 桥的集成测试用进程内的 `miyu-core`（开发依赖）装出厂的 onebot 包；拆出去以后核心的资源目录里没有 `packages/onebot/`，新仓库的测试夹具要拼一个资源目录：核心钉住那一版的资源，加上自己的包文件夹。

### 验收

- 接入QQ 原来的测试照旧过；新测试：扩展拉起时 `MIYU_PACKAGE_DIR` 是包文件夹（测试扩展记下环境变量那一步），自己装的包指家目录那一份；转交命令同样。
- 设了 `MIYU_PACKAGE_DIR`、资源目录里没有 `software/onebot/` 时，桥说的某一句不是空的（给人看的字读到了）。
- 去掉 `MIYU_PACKAGE_DIR` 的回退那一行，桥的「出厂参数读得到」测试照旧过（资源目录那一份）；去掉环境变量那一行，扩展测试红。
- `cargo xtask check` 八项；三台 CI；接入QQ 的会话照本机回放测具过一遍。
