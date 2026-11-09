## `advisor`

### 是什么

问顾问（设计 `69-旗舰顾问.md`，施工 ADV-1、ADV-2）：她卡住了，把问题交给旗舰池（`@flagship`）里的模型，同步等它答完，回答原文当工具结果交回。执行器照父会话这一刻的样子填好交给会话表，会话表造一个顾问会话（父会话的子会话，没有工具、没有人格、钉 `@flagship`，system 是 `core/advisor/system.txt`），把问题作为父会话发来的话送进去，等这一轮答完。和 `subagent` 不同：派子代理是派活，当场交回、它在后台跑；问顾问是问主意，等它答完才交回（设计 69 第二节）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/advisor.rs` | 参数、交给端口、结果和说法、`detail` |
| `crates/miyu-tool/src/advice.rs` | 名字 `ADVISOR`；问到了 `Advised`（回答原文、顾问会话）、没问到 `NotAdvised`（池里没有模型、没答成） |
| `crates/miyu-tool/src/agents.rs` | 派子代理的端口 `AgentPort` 多一样 `advise`（默认问不了） |
| `crates/miyu-session/src/agents/spawner.rs` | 执行器这一头：照父会话填好 `Advice`（层数、工作目录、场所、权限照父会话、只读打开），交给会话表的端口，记一行运行日志 |
| `crates/miyu-session/src/spawn.rs` | 会话表的端口 `SessionPort` 多一样 `advise`（默认问不了）、要的 `Advice` |
| `crates/miyu-session/src/advice.rs` | 顾问会话的 `origin`（`advisor`）、命令编号 `<父会话>/advisor/<调用>`；照命令编号认出顾问会话，不当没有任务编号的子代理记 `WARN`（`report.rs`） |
| `crates/miyu-session/src/open.rs`、`open/draft.rs` | 造顾问会话：快照是裸的（同整理会话，`miyu_policy::Snapshot::bare`），system 换成顾问那一份，进快照 |
| `crates/miyu-store/src/resources/advisor.rs` | 读 `core/advisor/system.txt` |
| `crates/miyu-endpoint/src/sessions/advisor.rs` | 会话表这一头：看旗舰池、找最近一次的顾问会话、造顾问会话、送问题、等答完；等到一半被掐掉的打断顾问那一轮 |
| `crates/miyu-endpoint/src/sessions/asked.rs` | 发一句、等由它开的那一轮结束、读回正文（施工 ADV-1 从 `dream.rs` 挪出来，整理会话共用） |
| `crates/miyu-endpoint/src/sessions/orphans.rs` | 收派到一半的空子会话时不收顾问会话 |
| `resources/software/basesystem/tools/advisor.json` | 说明和参数格式 |
| `resources/software/basesystem/advisor/*.txt` | 输出里给她看的两句 |
| `resources/core/advisor/system.txt` | 顾问会话的 system |
| `resources/software/basesystem/human/{zh,en,ja}.json` | 显示名、三种说法 |
| `resources/packages/miyu-base-advisor/package.toml` | 出厂包 `miyu-base-advisor`（基础系统组，排在 `miyu-base-subagent` 后面） |

### 对外的样子

访问类别 `read`：问一句什么都不改，顾问没有工具。没有声明范围（`scope`），哪种会话都有它：场所会话（群）也能调，只是问模型、不碰本机（设计 69 第三节第 6 条）。工具一直在工具面上，说明和参数格式是资源里的常量，不随旗舰池里有没有模型变（设计 69 第二节、AGENTS「工具面会话内字节恒定」）：池里没有可用模型的，调了照「出错」那一句说。

样本 `resources/software/basesystem/tools/advisor.json`：

```json
{
  "description": "Ask a stronger model for advice when you are stuck. It sees only your question, so include the context it needs.",
  "parameters": {"type":"object","properties":{"question":{"type":"string"},"continue":{"type":"boolean","description":"true: continue your last advisor conversation in this session."}},"required":["question"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `question` | 是 | 问题：原样作为父会话发来的话送进顾问会话，不加包装，不进 system（设计 69 第一节第 4 条：前缀不随问题变） |
| `continue` | 否 | 布尔：`true` 的接着这个会话最近一次的顾问会话说，顾问看得到上一次的问答；没有的照新开。不写、`null`、`false` 的新开一个 |

- 别的参数不认，也不报错（同 `subagent`）。

**顾问会话**（设计 69 第三节第 4、5 条）：

1. 父会话的子会话：`session.created` 带 `parent`、`depth`（父会话的加一），`by` 是父会话，`cause` 是 `<父会话>/advisor/<调用编号>`；场所、工作目录、加进来的目录、显示图片、关不关照父会话，权限照父会话这一刻的、只读开关打开；没人能确认（`attended` 假），不自动起标题。
2. `model` 是 `@flagship`，没有人格、`no_tools` 真，`origin` 是 `advisor`（「种类」，会话列表的那一项照原样带 `origin: "advisor"`，同终端集成开的 `terminal`）。
3. 快照是裸的（`miyu_policy::Snapshot::bare`，同整理会话）：没有工具、示范、记忆、知识库、技能，回合开始不注入事实；system 是 `core/advisor/system.txt` 去掉末尾的空白，进快照，之后照快照发，接着说时前缀不变。
4. 不是父会话的任务：父会话的日志里没有它的 `job.started`，它不向上回报（命令编号读不出任务编号，`report.rs` 认得出是顾问的，不记 `WARN`），收派到一半的空子会话时不收它（`orphans.rs`）；父会话删了它跟着删（照 `parent`）。用量照子会话记，算进父会话的那一棵（`session.list` 的 `tokens`）。
5. 会话列表：同子代理，`session.list` 照样列、带 `parent`；她的 `sessions` 只列主会话，不列它。

### 怎么走

1. 读参数：读不成的（没写 `question`、不是字符串、`continue` 不是布尔），交回参数不对的那一句，端口一次都不问。
2. 这一次调用没有派子代理的端口（`Call.agents` 是空的：测试里的假调用、脚本里调的）：交回没答成，原因 `no advisor here`。
3. 交给端口 `advise(question, continue)`：执行器照父会话填好交给会话表（`SessionPort::advise`）。
4. 会话表照这一刻的配置看旗舰池（`miyu_models::pools::pool`，成员一个都认不出的也算没有）：没有可用模型的什么都不造、不发，交回池里没有模型。
5. `continue` 的照会话列表的索引找父会话最近一次的顾问会话（`parent` 是它、`origin` 是 `advisor`，造得最晚的）；同一个顾问会话一次只答一个问题（会话表记着正在答的那几个）：找到的那一个别的调用正占着的（一步里并着调两次 `continue`），照新开，不然后一句排进前一句正在答的那一轮，等不到自己的那一轮（`kernel/session.md`「排队的消息」）。没找到的、没写 `continue` 的造一个新的（「顾问会话」第 1 到 3 条）。父会话已经不在会话表里的不造。
6. 以父会话的名义、编号 `<造它的命令编号>/ask` 把问题送进去，等由它开的那一轮结束（`sessions/asked.rs`）：答完的交回这一轮她说的正文（文字块照先后接起来，思考不要）和顾问会话的编号；被打断的原因是 `interrupted`；出错结束的原因是结束的原因加请求的错（例如 `error: <供应商的原话>`）；会话停了、拒了的原因照会话表的说法。
7. 等到一半被掐掉（她那一轮被打断、来了一句话）：在别的任务里打断顾问那一轮（命令编号 `<造它的命令编号>/stop`），不白花旗舰模型的钱。
8. 问到了：交回回答原文，末尾不加东西；说法 `advisor/answered` 带 `session`；`detail` 是 `{"session": "<顾问会话>"}`，头照它点进去看问答（施工 ADV-3）。

### 样子

问到了，交回的就是顾问的回答原文：

```text
Take A before B everywhere.
```

给她的字都在 `resources/software/basesystem/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节：

| 什么时候 | 文件 | 原文 |
|---|---|---|
| 旗舰池里没有可用模型 | `advisor/no-model.txt` | `No model is set in the flagship pool.` |
| 顾问没答成 | `advisor/failed.txt` | `The advisor could not answer: {reason}` |

顾问会话的 system（`resources/core/advisor/system.txt`，登记在同一张表里）：

```text
You are an advisor to another AI agent. You see only the question it sends you, not its conversation or files. Give specific advice it can act on. If the question lacks something you need, say what is missing.
```

### 出错

出错的结果都标成出错，不带 `detail`。

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 参数不对 | `common/bad-args.txt` | `common/bad-args`，字段 `error` |
| 旗舰池里没有可用模型 | `advisor/no-model.txt` | `advisor/no-model` |
| 没有端口、造不成会话、问不进去、这一轮出错、被打断 | `advisor/failed.txt`，`reason` 换成原因 | `advisor/failed`，字段 `reason` |

### 给人看的字

显示名：顾问（Advisor、アドバイザー），后面跟 `question` 的值；符号 `?`。

| 说法 | 中文 | 英文 | 日文 |
|---|---|---|---|
| `advisor/answered`（`session`） | 答了 | Answered | 回答しました |
| `advisor/no-model` | 旗舰池里没有模型 | No model in the flagship pool | フラッグシッププールにモデルがありません |
| `advisor/failed`（`reason`） | 没答成：{reason} | Could not answer: {reason} | 回答できませんでした：{reason} |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/advisor.rs` | 声明 `question`、`continue`，说明是设计的原文、访问类别是读；说明、参数格式造几次都一字不差、不带池和模型的名字；回答原文交回、`detail` 带顾问会话；`continue` 交给端口；池里没有模型、没答成、没有端口各照那一句说；少了问题的端口不问；三种说法、三种语言的显示名都在 |
| `crates/miyu-endpoint/tests/advisor.rs` | 真核心走一遍：旗舰池没有模型的交回那一句、不发请求、不造会话；有的造顾问会话（父会话造、第 1 层、`@flagship`、无人格、无工具），问题是那一次请求唯一的一条消息、system 是顾问那一份、没有工具，回答原文交回；`continue` 接着最近的那一个（第二次请求里看得到第一次的问答），不写的新开；没有可接的照新开；一步里并着两次 `continue`，一个接着、一个新开，两个都答得回来 |
| `crates/miyu-session/src/advice/tests.rs` | 顾问的命令编号认得出、和子代理的分得开；只有 `origin` 是 `advisor` 的算顾问会话 |
| `crates/miyu-endpoint/src/sessions/orphans/tests.rs` | 收派到一半的空子会话时不收顾问会话，子代理照收 |
| `crates/miyu-basesystem/tests/budget.rs` | 工具面的预算 |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- 设计 `69-旗舰顾问.md` 第一节到第四节。
- `agents.md` 第一条（子会话怎么造）、`subagent.md`（端口、说法的写法）。
- `26-提示词.md` 第十节（登记簿）。

### 还没有的

- 时间线里点进顾问那一步看问答：施工 ADV-3，归界面的会话。
- 工具名验收时问一句定 `advisor` 还是 `get_advice`（设计 69 第三节第 1 条）。
