## 施工单 S-6 上：接入QQ 只依赖门面、从包文件夹读自己的文件

状态：待施工。设计 `32-仓库拆分.md` 第九节；项目主人 2026-10-16 定接入QQ 拆出核心仓库、单独发、自己装。接入QQ 的会话 10-16 回话：没有在途的活，随时可以拆。

### 起因

拆出去以后桥照 git 依赖钉核心，要和两个界面一样只借门面 `miyu-client`（第二节第 4 条：先在一个仓库里收紧）。自己装的包住在家目录的 `packages/onebot/`，资源目录里没有 `software/onebot/`，桥得能找到自己的包文件夹。

### 这一步做什么

1. **`MIYU_PACKAGE_DIR`**：核心拉起扩展（`extensions/supervise.rs`）、命令行转交包的命令（`miyu-cli` 的 `packages::forward`，Windows 那一份同样）时多设这一个：这个包的文件夹（出厂的在资源目录、自己装的在家目录，哪一层就是哪一层）。`MIYU_HOME`、`MIYU_RESOURCES` 照旧。作者说明第四节第 1 条、`extensions.md` 第 122 行那一条一起改。
2. **包文件夹带数据**：`resources/software/onebot/` 整个挪进 `resources/packages/onebot/data/`（`git mv`）；桥的 `tuning`、`rules`、`texts`、`settings`（读自己清单的出厂值）照 `MIYU_PACKAGE_DIR` 读，没设的（老核心、手跑）退回资源目录的 `packages/onebot/`。给人看的字的编号照旧是 `software/onebot/<哪一句>`，只换读的地方。登记簿门禁的四份豁免、设计 26 第十节里的路径照新位置改。
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

### 验收

- 接入QQ 原来的测试照旧过；新测试：扩展拉起时 `MIYU_PACKAGE_DIR` 是包文件夹（测试扩展记下环境变量那一步），自己装的包指家目录那一份；转交命令同样。
- 去掉 `MIYU_PACKAGE_DIR` 的回退那一行，桥的「出厂参数读得到」测试照旧过（资源目录那一份）；去掉环境变量那一行，扩展测试红。
- `cargo xtask check` 八项；三台 CI；接入QQ 的会话照本机回放测具过一遍。
