## 施工单 DSR-3：DeepSeek 内置联网工具的激活判断和工具面替换

状态：做完，等验收（分支 `step/DSR-3`）。设计 `72-DeepSeek内置联网工具.md` 第五节第 3 步。

### 为什么

施工 DSR-2 加了「联网服务」的选项 `deepseek-responses`，但是选中它还没有效果。这一步让它生效：满足条件时，请求的 `tools` 末尾声明 DeepSeek 的两个服务端工具，工具面里去掉 Miyu 自己的 `web_search` 和 `web_fetch`。

现在的代码有两处不够用：

- 工具面钉在策略快照里，只有换人格、换预设、工具目录换代这些事会在回合开始时重拼，换模型不会重拼。
- 请求里的 `tools` 只能带 `function` 工具。

### 改成什么样

**数据**：`resources/models/profiles.toml` 的 `[providers.deepseek]` 多一格 `search`，写明对应「联网服务」的哪个值（`deepseek-responses`）、要求哪种驱动（`openai-responses`）、哪件 Miyu 的工具换成哪件供应商侧的工具（`web_search` 换成 `deepseek_web_search`，`web_fetch` 换成 `deepseek_read_url`）。

**模型这边满不满足条件**（`miyu-models`）：供应商的档案写了 `search`，地址是官方地址（沿用 `provider_key` 现有的认法），这个模型走的驱动和档案写的一样，思考没有设成关。模型池要每个成员都满足。

**判断**（`miyu-policy` 的 `provider_search`，纯逻辑）：回合开始时，会话的模型端口报告模型这边满不满足条件。

- 不满足：快照里不钉任何东西，工具面是原样。
- 满足，并且快照里钉着的模型引用和现在一样：沿用钉着的「选没选」。所以会话中途只改「联网服务」不会改变工具面。
- 满足，但是快照里没有钉，或者模型引用变了：读这一轮的「联网服务」，重新判断。
- 换哪几件每次按这一刻的工具面重算：工具面里原本有的才换。

**钉在哪**：策略快照多一格 `provider_search`（判断时的模型引用、选没选、换掉的几件）。模型这边不满足条件的会话没有这一格，快照的字节和以前一样。

**工具面**（`miyu-session` 的 `actor/persona/search.rs`）：回合开始重拼时，钉的结果变了就换快照。换掉的工具从工具面里拿掉；不再换的工具按这一刻的目录和预设补回来。

**发请求**（`miyu-drivers`、`miyu-session` 的路由）：`Call` 多一格 `hosted`（供应商侧工具的名字）。路由在发给某个端点之前，确认这个端点满足条件，才把快照钉着的名字交给驱动；`openai-responses` 驱动把每个名字写成 `{"type": "<名字>"}` 接在 `tools` 的末尾，其他驱动不处理这一格。只有会话自己的请求（主请求，以及和它共用前缀的压缩请求）声明；起标题、回顾、看图这些辅助请求不声明。

**明确写出思考强度**：声明了供应商侧工具、这一次又没有定思考强度时，路由把思考强度设成档案里 `search.effort` 写的那一档（`high`）。原因是实测发现，普通工具和联网工具一起声明时，请求里不明确写思考强度，DeepSeek 不会执行搜索（设计 72 第三节）。

**`shell` 的说明跟着改**：`shell` 这类跑命令的工具，说明末尾有一段「脚本里可以用 `miyu call` 调用哪些工具」的名单。换掉搜索工具时，这段名单按新的工具面重写（`agents/clicall.rs` 的 `redescribe`）；名单里举例的那一行如果调用的工具不在名单里，就不带这一行（`miyu-call` 的 `section.rs`）。不这样做的话，AI 会按旧的名单和例子去调用已经换掉的 `web_search`，白白失败一次。

蓝图 `policy.md`、`models.md`、`drivers/openai-responses.md`、`session/tools.md` 和设计 72 第二节一起修改。

### 不做什么

- 不接住 DeepSeek 返回的 `web_search_call`（第 4 步）。这一步之后搜索能进行，模型能根据搜索结果回答，但是日志和界面上还看不到搜索步骤，下一轮也不会把搜索记录带回去。
- 新会话不在创建时判断，而是在第一轮开始时判断。这时还没有发过请求，不影响缓存；`session.tools` 在第一轮之前预览到的仍然是 Miyu 自己的工具。
- 预设找不回来的旧会话、裸会话和 fork 出来的子代理不重新判断，沿用快照里钉的结果。

### 已知的局限

激活之后，如果某一次请求因为出错换到了不满足条件的端点（钉住的模型池出错换了成员，或者退回 `models.chat`），这一次请求既没有联网工具的声明，也没有 Miyu 自己的搜索工具。下一轮开始时会重新判断并恢复。

### 验收

- 修改之前测试是红的。
- 缓存守卫：同一个会话的两次请求，`tools` 数组逐字节相同；会话中途改「联网服务」之后仍然相同。
- 门禁、Windows 目标的 clippy、三个平台的 CI 全部通过。
- 实测：用 DeepSeek 官方密钥在沙箱里确认请求成功、模型能搜索。

### 做下来的

- 改前红：这一步新加的测试在修改之前无法编译，因为它们用到的类型、字段和函数都还不存在。会话层面的五条测试（`actor::persona::search_tests`）里有一条在第一版代码上失败过：模型满足条件但是「联网服务」没有选这一项时，判断结果没有被保存，之后中途选上会立刻生效。把「判断结果变了」算作要保存的改动之后通过。
- 测试：
  - `miyu-policy` 的 `provider_search::tests`（6 条）：判断的规则、换哪几件、钉的东西写进快照的样子。
  - `miyu-session` 的 `actor::persona::search_tests`（5 条）：激活后工具面没有两件搜索工具、下一轮不变、中途改设置不变、换模型重新判断、模型不再满足条件时补回来、预设只关一件时只换另一件。
  - `miyu-session` 的 `route::search::tests`（3 条）：只有满足条件的端点才声明；没有定思考强度时用档案写的那一档。
  - `miyu-session` 的 `agents::clicall::tests`、`miyu-call` 的 `section::tests`：名单重写和例子那一行。
  - `miyu-models` 的 `provider_search::tests`、`miyu-core` 的 `models::tests`、`miyu-drivers` 的 `openai_responses_hosted`：档案的数据、模型这边的条件、驱动写出的 `tools`，以及留着历史的编码在 `hosted` 变化时不用旧的那一截。
- 驱动和档案的那一半由子代理完成，我检查了它写的驱动代码，并重新运行了相关的测试。
- 实测一（2026-11-19，抓请求原文）：把 DeepSeek 供应商的地址写成 `http://api.deepseek.com:18080`（主机仍然是官方的，所以算官方地址），让核心经本机的一个记录请求的代理发出，密钥是假的。抓到的请求里 `tools` 一共 24 件：22 件 `function` 工具，末尾是 `{"type":"deepseek_web_search"}` 和 `{"type":"deepseek_read_url"}`；没有 `web_search` 和 `web_fetch`；`shell` 说明末尾的名单是 `history, session_usage, sessions`，没有举例的那一行。
- 实测二（2026-11-19，DeepSeek 官方接口，项目主人的密钥，沙箱数据根）：
  - 第一版（没有明确写思考强度、`shell` 的名单没有重写）：AI 先运行 `miyu call web_search`，失败后有一次用上了服务端搜索，另外几次改用 `curl`，还有一次回答「我没有自带的联网搜索工具」。
  - 最终版：「联网服务」选 `DeepSeek Responses 接口内置`，配置里没有定思考强度。问「Rust 语言最新的稳定版本号和发布日期」，AI 在一次请求里完成搜索并回答 `1.100.0`、2026 年 11 月 12 日和官方公告的网址，没有调用 `shell`；这次请求的输入是 21,752 个 token（缓存命中 75%）。
  - 这个会话的工具面是 22 件，没有 `web_search`、`web_fetch`；中途把「联网服务」改成 `auto` 之后再问一句，工具面仍然是 22 件。改成 `auto` 之后新开的会话是 24 件，包含这两件。
- 实测中看到的、要等第 4 步解决的现象：搜索记录还没有带回下一轮，所以下一轮问 AI「你刚才是怎么查到的」，它不记得自己搜索过，有一次甚至回答「上一条的内容是我编造的」。第 4 步让驱动接住 `web_search_call` 并原样带回之后才会正常。**建议这一步和第 4 步一起合进 main。**

### 怎么实测验收

需要一个 DeepSeek 官方的密钥。

1. 把 `MIYU_HOME` 指到一个试验用的空目录，运行 `miyu core start`，在 `etc/config.toml` 里写入：

   ```toml
   [providers.deepseek]
   key = { env = "DEEPSEEK_API_KEY" }
   driver = "openai-responses"

   [models]
   chat = "deepseek/deepseek-flash"

   [net.search]
   source = "deepseek-responses"
   ```

2. 运行 `miyu ask "搜索一下 Rust 语言最新的稳定版本号和发布日期，一句话回答并给出来源网址"`。AI 应该直接回答最新的版本号和网址，过程中不调用 `shell`；末尾的用量一行里输入在一万个 token 以上（搜索到的网页内容算在输入里）。
3. 运行 `miyu config set net.search.source auto`，再运行 `miyu ask -c "只回答两个字：收到"`。这个会话仍然没有 `web_search`、`web_fetch` 两件工具（可以用协议的 `session.tools` 查看）。
4. 新开一个会话（不加 `-c`），它的工具面里有 `web_search` 和 `web_fetch`。
