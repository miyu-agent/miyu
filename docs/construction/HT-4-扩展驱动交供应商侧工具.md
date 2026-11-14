## HT-4 供应商侧工具：扩展驱动交得上来

状态：施工完，待 core1 审（2026-11-14）。设计 `71-供应商侧工具.md` 第三节第 6、7 条，第七节第四行；设计 66 第四节第 1 条、第五节第 2 条跟着加。
接在 HT-1、HT-2、HT-3 后面。只有技术细节，直接做。

### 目的

扩展写的模型驱动（`antigravity` 这类借 agent CLI 订阅的包）能把「供应商自己跑了一件工具」报上来：线上多两种写法，清单里写明
会让供应商自己跑哪些工具，装包时人看得见。这一步做完，从扩展进程到日志、到视图是通的。

### 蓝图改哪几节

- `extensions.md`「扩展提供的模型驱动」，协议参考 `extensions.md`（中英）的 `driver.delta`：
  1. `start` 的 `kind` 多一种 `{"hosted_tool": {"name"}}`；增量多一种 `{"outcome": {"index", "text", "failed"?}}`（`failed`
     不写当假）。照累加器的规矩查（只给供应商侧工具的块、一块至多一次、收全以前），不对的同别的坏增量：`driver.cancel`、
     这一次当 `unclassified`。`private` 照旧能给这种块，记成这个驱动的家族。
  2. 结果的字核心截到 64 KiB；扩展自己先截更好（少过一次进程）。
- `packages.md`（清单）、作者文档 `manifest`（中英）：`[[driver]]` 多一格 `hosted = ["search_web", "read_url_content"]`，
  可以不写（空的）。每个名字 1 到 64 个字节、不带控制字符、一段里不重；写错的清单读不进（同 `[[driver]]` 别的格，`Code` 用
  现成的那一种）。
- `packages.md`「装包确认」、`package-pages.md`、协议参考里装包预览的回应（中英）：驱动那一项带上 `hosted`；命令行的确认屏在
  驱动名下面多一行「这个驱动会让供应商自己跑：search_web、read_url_content」（中英各一句，`miyu-cli/src/language/confirm.rs`
  的体例），没声明的不写这一行。
- `extensions.md`、`log.md`：没写在 `hosted` 里的名字交上来，核心照收、照画，记一行 `WARN`（`driver hosted tool undeclared`，
  带包、驱动、工具名；同一个驱动同一个名字一个进程里只记一次，免得刷屏）。端口的 `Declared`（`route/driver_port.rs`）多带
  `hosted`，执行器照它查。
- `extensions.md`「查编码稳不稳」（`driver.check`，`miyu dev driver-check`）：带私有数据的那份样本里多一块这个驱动自己家的
  `hosted_tool`（带结果、带私有数据），稳定区照旧要逐字节相同。
- 作者文档「写一个模型驱动」（中英）加一节「供应商自己跑的工具」：怎么报；想在下一次请求里原样收回自己交的块，交的时候带
  一份 `private`（内容随意），没带的核心换成一段字交回来；**会改东西的原生工具（跑命令、写文件）不许报成供应商侧工具，要
  关掉、改走核心的工具**；清单 `hosted` 要写全。
- 设计 66 第四节第 1 条的清单例子加 `hosted` 一行、第五节第 2 条的增量列表加两种（指到设计 71）。
- 测试扩展（`miyu-test-extension`）：驱动多一种答法 `driver:hosted`（开一块供应商侧工具、参数、结果、收，再说一句正文），清单
  里声明它交的那个名字；再一种 `driver:hosted-undeclared` 交一个没声明的名字。

### 不做什么

- Anthropic 驱动：HT-5。`antigravity` 包本身：在包的仓库里做。
- 核心拦没声明的名字：不拦（设计第三节第 7 条：挡不住供应商那边跑什么，只照实显示）。
- 网页的装包确认页：别的仓库，回应里带了 `hosted`，告诉网页的会话。

### 验收

1. `miyu-session` `route/extension/wire/tests.rs`、`tests.rs`：两种新写法读得出；`failed` 不写当假；写错的（多一格、
   `outcome` 没 `text`）拒；结果给了正文块、来两次的 → `driver.cancel`、`unclassified`；带 `private` 的记成扩展的家族；
   没声明的名字照收、记一次 `WARN`，再来一次不再记；声明了的不记。
2. `miyu-config`：`hosted` 读得出、不写是空的；空名字、太长、重复、不是字符串数组的清单读不进，报在那一行。
3. `miyu-endpoint` 真核心、真扩展进程（`tests/extension_driver_call.rs` 旁边加）：`driver:hosted` 走一轮 →
   `message.assistant` 里有 `hosted_tool` 块（名字、参数、结果、私有数据的家族是 `<包>/<驱动>`），回合结束、一件工具都没跑；
   `view.page` 里是一步工具（`hosted` 真、`via` 是这家供应商、状态 `ok`、`output` 是结果）——HT-3 留下的那条在这里补；
   紧接着第二轮的 `driver.call` 里，上一轮那块原样在 `request` 里（自己家、带私有数据）；换一个内置驱动的模型再发一轮，
   请求里它是那段 `<hosted-tool …>` 的字。
4. 装包预览的回应带 `hosted`；命令行确认屏那一行中英各对一遍样本。
5. `driver.check` 的样本里有这块；测试扩展过得了检查。
6. 手搓变异 10 个左右全红。
7. 门禁全过（「文档」「日志」两项要过：参考中英都有条目、`log.md` 登记），分支三平台 CI 全绿（碰子进程，先过 CI 再交）。

### 风险

- 线上写法是 `deny_unknown_fields`：老核心收到新写法会当坏增量拒掉。包要用新写法就在清单里写最低核心版本（X-31 现成的）。
- `WARN` 只记一次的表放在哪：照执行器现成的「只说一次」的做法，没有现成的就放端口那一头（`miyu-endpoint` 的 `Drivers`），
  别放内核。

### 做下来的

- 装包预览、`package.info` 的 `contains` 里没把 `hosted` 塞进 `drivers` 那一项，另起一格 `hosted`：「驱动名 → 工具名的数组」，
  没声明的驱动不列、一个都没声明的整格不带。`drivers` 照旧是名字的数组，现在的网页和命令行不用跟着改。
- 确认屏那一行写明是哪个驱动：`模型驱动 ag 会让供应商自己跑：search_web、read_url_content`，英文
  `model driver ag has the provider run on its own: search_web, read_url_content`。
- 「只说一次」的表放在执行器现成的那张（MD-3 的 `sent` 对不上只说一次也在那里，`route/extension/wires.rs`），键是（驱动家族，
  工具名）。名字是扩展交的、能无限造，表有上限：每个驱动最多记 64 个名字，长过 64 个字节（清单 `hosted` 的上限，同一个常量
  `HOSTED_NAME_BYTES`）的不进表；满了又来新的、或者名字太长的，这个驱动只再记一行 `WARN driver hosted tools undeclared, more
  not listed`（不带名字），之后都不记。增量照收，一条不少。`miyu-session` 里只查表交回的说法和走完以后表的样子，不查日志行：
  同一处 `warn!` 别的测试线程也走，线程上装的订阅者收不到（试过 `rebuild_interest_cache`，并行跑照样收不到）。
- 清单 `hosted` 写错的用现成的 `bad_driver`，报在那一段 `[[driver]]` 的行上，详情 `driver.<驱动名>.hosted`。
- 施工单写的 `package-pages.md` 没有装包确认那一节，没动；装包确认改在蓝图 `packages.md`、`cli/pkg.md`，协议参考改在
  `packages.md`（中英）。顺手补上 MD-1 漏写的两处：参考里预览回应的 `drivers` 一格、`cli/pkg.md`「包含」里的「模型驱动 <名字>」。
- 真核心、真扩展走一遍（`miyu-endpoint` `tests/extension_driver_hosted.rs`）：日志里的块、视图的一步（HT-3 留下的那条在这里
  补上）、第二轮请求里原样在、换到内置驱动的模型以后是那段字；没声明的 `WARN` 只一行另成一个测试程序（要装日志订阅者）。
- 手搓变异跑了 10 个全红（每个要重编端点、排整机编译锁，十来分钟一个）：`failed` 不写当成真、`hosted_tool` 的名字丢了、读成
  工具调用、声明了的也记、没声明的不记、结果不过累加器的查、空名字不拒、确认屏不写那一行、预览不带 `hosted`、`WARN`
  每次都记。写好没跑的 7 个：样本少私有数据、家族记错、只说一次的表不记、重复名字不拒、64 字节也拒、端口不交 `hosted`、
  结果交不出去。
