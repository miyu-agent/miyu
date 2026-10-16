## 施工单 S-6 下：核心仓库删掉接入QQ

状态：做好了（2026-10-17，已合 1dd7cd92，三平台 CI run 38944278572 绿；本机门禁八项，Linux 发布包里没有 `miyu-onebot`）。设计 `32-仓库拆分.md` 第九节第 5、6 条。S-6 中开好了 `miyu-agent/miyu-onebot`（f54e4fb，钉核心 7384e880，三平台 CI 绿；接入QQ 的会话照新仓库跑过回放，26768 条两种口径逐字节一样），从此桥在那边改。

### 这一步做什么

1. **删**：`crates/miyu-onebot/`、`crates/miyu-chat/`、`resources/packages/onebot/`（`git rm`）。工作区的成员、`Cargo.lock` 跟着收。
2. **核心发布包不带桥**：发布、打包流程（`xtask` 里打包的那一段、`.github/workflows/` 的发布）不再编、不再装 `miyu-onebot`；有写死它的地方一起删。
3. **测试改夹具**（第六条）：借出厂的 `onebot` 包当例子的测试（端点的软件包测试、`miyu-core` 的配置项测试、配置样本），改成测试自己放的夹具包（照现有测试扩展的写法：清单写在测试里或 `tests/` 下的夹具目录）。**只是拿 `onebot` 当一个名字的不动**（场所编号 `qq:…`、平台前缀、系统账号名这些）。
4. **登记簿**：设计 26 第十节里接入QQ 那几段（S-6 中已经照抄到新仓库的 `docs/ledger.md`）删掉，留一行指路；`xtask` 登记簿门禁里给 `packages/onebot/data/` 的豁免、只为它写的测试用例删掉或改成中性的例子。`cargo xtask prompts` 重新生成 `docs/blueprint/prompts.md`。
5. **配置样本**：样本里包的配置项随桥走，照只编核心的程序重新生成六份样本、同步蓝图 `config.md` 里嵌的几块。
6. **图纸**：`docs/blueprint/onebot.md`、`chat.md` 换成一行指路（「在 `miyu-agent/miyu-onebot` 的 `docs/blueprint/` 下」），同 S-5 第 2 条；别的图纸里指向它们的链接照旧写文件名，前面加「接入QQ 仓库的」。`01-架构.md` 第九节层序表、分层门禁 `TIERS` 里去掉这两个 crate。设计 18、`O-*` 施工单、施工图上的 O 线芯片不动（历史）。
7. **作者说明、`heads.toml`**：作者说明里拿接入QQ 当「核心自带的扩展」举的例子改成「单独装的扩展」；`heads.toml` 本来就不带它，确认一下。

### 不做什么

- 不改核心的行为、不改协议。门面的 `http`、`tool` 两个 feature 照留（新仓库在用）。
- 不改 `miyu-onebot` 新仓库。

### 验收

1. `cargo xtask check` 八项；`git grep -n 'miyu-onebot\|miyu_onebot\|miyu-chat\|miyu_chat'` 只剩历史文档（施工单、设计 18、施工图、`next-release-note` 之类）和指路的行。
2. 删了以后核心照旧编得出四个平台的发布包（本机编 Linux 的看一眼包里没有 `miyu-onebot`）。
3. 三平台 CI（删了 crate、改了打包，先推 `step/` 分支）。
4. 新仓库照钉住的那一版不受影响（它钉 7384e880，核心这边删东西不影响它）；以后升级核心时它照新 rev 过 CI。
