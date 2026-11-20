## PV-4 软件包自带供应商

状态：施工完，待验收（2026-11-17）。项目主人 2026-11-17 定（本机验收第 20 条：装了 Antigravity 包不知道怎么切到它的模型）。主会话派工时叫 PV-1，和已有的 PV-1（「下载软件详情页的提供」）撞了号，改叫 PV-4。

### 目的

- 包能在清单里声明自带的模型供应商；装上、开着的时候供应商列表就有它，`/model`、`models.chat` 直接切到它的模型，不用自己抄配置。
- 包带的供应商不写配置文件：卸掉、关掉就没有；人改它的设置写在配置文件里、盖住包给的；人自己已有同编号的，人的优先。

### 蓝图改哪几节

- 设计 `66-扩展提供模型驱动.md` 第四节第 2 条：原来只认主人手写的 `[providers.<id>]`（包不许写 `providers.*`），改成「包能自带供应商，不能改人的」，写清理由和规矩；第十四节「已定」加第 3 条。
- `config.md`「最终值和来源」：层的先后多「包这一层」，`layer` 表的 `package` 多「软件包带的供应商」，新加一段「包这一层」。
- `models.md`：供应商的键多 `package`；资料的来源多 `package`。
- `packages.md`：种类能写的表（扩展、`bundle` 多 `[providers.<编号>]`），新加一段「自带的供应商」。
- 协议参考 `config`（中英）：来源 `{"layer": "package", "package"}`。作者文档 `manifest`（中英）：新加 `[providers.<编号>]` 一节，`[[driver]]` 一节提到它。

### 定了的

- 清单写 `[providers.<编号>]`，格子同配置里的 `[providers.<编号>]`（`name` 三种语言、`driver` 必写、`base_url`、`catalog`、`price_multiplier`、`local`、`cache`、`models`），复制粘贴就对得上；`key` 不收（key 是人的）。`driver` 只能是内置的三种，或者自己这个包 `[[driver]]` 里的（指别的包的不收：不让一个包把对话交给另一个包）。
- 在配置里是一层：默认值之上、配置文件之下（`miyu_config::merge::Packaged`）。合的规矩、来源都照现成的一套，模型列表、路由、`check`、`config.get` 不用另写分支。
- 人盖在包带的那一家上的，配置文件里那一段带 `providers.<编号>.package = "<包>"`（`config.set` 改包带的那一家时核心顺手写上）。不带的同编号是人自己的：包带的整家不出现、`miyu check` 提示。不照「写没写驱动」区分：人原来只写了 key、驱动照目录推的那一家（DeepSeek、Anthropic 常这样配），同编号的包一装就会把 key 交给包的驱动（设计 66 第九节第 2 条要防的）。包关了、卸了，带 `package` 的那一段整段不算（照旧写在文件里，包回来接着用）。
- 名字照系统配置里定了的界面语言挑（没定、`auto` 的照核心环境的语言，同 `ui.language` 的 `auto`）：配置的值不分连接。
- 包这一层变了推 `config.changed`（`layer: system`、`via: package`），头照它重读模型列表。

### 不做什么

- 不改 Antigravity 包：清单写法交回给 agy-cli 会话。
- 不改终端、网页：模型列表本来就照 `model.list`，名字的来源多一种 `package`（`from: "package"`）。设置页画不画「来自软件包」是头的事。
- 包不能带 key，不能改人自己的供应商、池、`models.chat`。

### 验收

1. 改前红：`miyu-config` `package::provider::tests` 三条（main 上不认 `[providers]` 这张表，`unknown_table`）、`merge::tests::the_package_layer_sits_between_defaults_and_the_system`（main 上没有包这一层，编不过）；`miyu-endpoint` `config::provided::tests` 五条、`package_providers` 两条（main 上包带的这一家不出现）。
2. 跟着改的测试：各处 `Layers { … }` 多 `package: None`；`package::fields::tests` 照作者文档查 `[providers]` 那一节。
3. 门禁十一项、`random::` 长跑、Windows 版 clippy、`cargo xtask prompts`、`cargo xtask logs`。
4. 实测（沙箱数据根，假扩展驱动）：装带 `[providers.agy]` 和假驱动的本地包；`model.list` 出现这一家和驱动列的模型，`config.get providers.agy.driver` 来源 `package`；切到它的模型说一句拿到假驱动的固定字；`miyu config set providers.agy.name …` 以后来源 `system`、配置文件多 `package = "…"`；关掉包后这一家没了。网页头的模型列表里看得到这一家（截图）。
   - 2026-11-17 实测过（调试版核心，测试用的扩展程序当假驱动，`systems = ["linux", "macos"]`）：装上就有，`miyu config explain providers.agyd.driver` 印「软件包 agy-demo ← 生效」；`miyu ask --model agyd/ag-pro` 回「echo: …」；`miyu config set providers.agyd.name` 以后配置文件多 `package = "agy-demo"`、名字来源换成系统配置；关掉包问它的模型报「模型或模型池不存在」，再开回来名字照旧；去掉 `package` 以后 `miyu check` 警告「软件包 agy-demo 带的供应商 agyd 没有加上：配置里已经有你自己的 agyd……」。网页头（miyu-web main 的演示桥）模型菜单列出 `ag-lite`、`ag-pro`「Antigravity（演示）」，发一句回「echo: …」。

### 风险

- 编号撞了人自己的：包带的不出现，`miyu check` 说清怎么办（删掉自己那一段）。作者文档建议编号带包名。
- 人在配置文件里手写盖包的那一段忘了写 `package`：当成人自己的，包带的那一家不出现，`miyu check` 提示。
