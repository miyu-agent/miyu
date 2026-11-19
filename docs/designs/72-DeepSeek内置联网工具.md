# 72 DeepSeek Responses 接口内置的联网工具

> <sub>本文档由 Anthropic 的 Claude Opus 5.5 生成。</sub>

状态：定稿（2026-11-19，项目主人审定）。接在 71「供应商侧工具」后面。

## 一、要解决什么

DeepSeek 官方的接口自带两个联网工具：`deepseek_web_search`（搜索网页）和 `deepseek_read_url`（读取用户消息里的链接）。请求里声明这两个工具以后，DeepSeek 在自己的服务器上完成搜索、打开网页、在网页里查找文字，最后直接交回回答，Miyu 这边不需要执行任何东西。

它和 Miyu 自己的网络搜索工具相比，好处是搜得更深、更快：一次请求里会自动搜索好几轮、读好几个网页，最多约 20 次调用。2026-11-19 用项目主人的 DeepSeek 密钥实测，`deepseek-flash` 一次请求搜索 5 次、查到 17 个来源，6 秒答出当天的新闻并给出网址。

它不是省钱的办法。DeepSeek 不按次收这两个工具的费用，只按模型 token 计费，但搜到的网页内容会放进上下文。实测那一次输入 11473 个 token、输出 567 个，按 flash 的价格算，高峰时段约 0.027 元，闲时约 0.014 元；而 DeepSeek 的搜索接口每次 0.005 元。所以这个功能是给想要搜得更深的人用的可选项，出厂不打开。

`web_search`（不带 `deepseek_` 前缀）是 DeepSeek 的另一个工具，只有 `deepseek-v4-pro` 支持，`deepseek-flash` 会静默忽略。本设计用的是所有模型都支持的 `deepseek_web_search` 和 `deepseek_read_url`。

## 二、做法

### 1. 设置：「联网服务」多一个选项

「网络搜索」设置里的「联网服务」（`net.search.source`）现在有 `auto`、`exa`、`anysearch`、`deepseek`、`tavily`、`brave`、`searxng` 几个选项。加一个：

| 值 | 界面上的名字 | 说明 |
|---|---|---|
| `deepseek` | DeepSeek 搜索接口 | 现有的选项。Miyu 的搜索工具调用 DeepSeek 的搜索、抓取接口，按次计费。 |
| `deepseek-responses` | DeepSeek Responses 接口内置 | 新加。对话模型是 DeepSeek 官方、走 Responses 接口时，由模型自己搜索网页、读取链接；搜索更深入，但每次搜索会多用约一万个 token。 |

出厂仍然是 `auto`，不选就不会生效。

### 2. 什么时候激活

选了「DeepSeek Responses 接口内置」以后，下面四条同时满足才激活：

1. 当前对话的模型用的是 DeepSeek 这一家供应商；
2. 这家供应商的地址是 DeepSeek 官方地址（`https://api.deepseek.com`），经过中转站的不算；
3. 这家供应商走的是 Responses 接口（`openai-responses` 驱动）；
4. 这个模型的思考没有设成关。普通工具和联网工具一起声明时，DeepSeek 要求开着思考，否则返回 400（见第三节的实测）。

模型是模型池时，池里每个成员都满足上面几条才算满足。

激活以后：

- 发给 DeepSeek 的请求里，在 `tools` 数组末尾按固定顺序加上 `{"type": "deepseek_web_search"}` 和 `{"type": "deepseek_read_url"}`；
- 请求里明确写出思考强度。会话或配置里没有定思考强度时，请求写 `"reasoning": {"effort": "high"}`（DeepSeek 默认的强度）。不明确写的话，DeepSeek 不会执行搜索（见第三节的实测）；
- `shell` 工具说明末尾「脚本里可以调用哪些工具」的名单跟着去掉这两个工具，名单里举例用的 `miyu call web_search` 那一行也不再出现，避免 AI 去调用已经换掉的工具；
- 工具面里去掉 Miyu 自己的网络搜索（`web_search`）和抓取网页（`web_fetch`）两个工具，避免两边都搜。
- 替换是一对一的：工具面里原本有 `web_search` 时才声明 `deepseek_web_search`，原本有 `web_fetch` 时才声明 `deepseek_read_url`。预设只关掉其中一个功能时，另一个照常替换。

没有激活时（比如换成了别家模型、用的是中转站、或者 DeepSeek 供应商走的是 OpenAI 兼容接口），按「自动」的顺序使用别家搜索服务，AI 照样能搜索。

**选这个选项时弹一句提示**：在终端或网页里选「DeepSeek Responses 接口内置」时，不检查当前的供应商设置，直接弹出一句提示「这个搜索服务只适用于 DeepSeek 官方的 Responses 接口」，只有一个「知道了」按钮。选项照常保存。DeepSeek 供应商的接口要不要改成 Responses，由用户自己到编辑供应商里改。

预设里关掉了「网络搜索」时，这一项不起作用，两种搜索都不会发给模型。

### 3. 什么时候重新判断（缓存）

工具面在一个会话里必须保持逐字节不变，否则会打断缓存。所以：

- **换模型时当场重新判断**。换模型本来就会让缓存断掉：供应商的缓存是按模型存的，新模型那边没有这段对话的缓存。所以在换模型的那一刻顺便换工具，不会多断一次。
- **会话中途改了「联网服务」设置**：从下一个新会话开始生效，或者等这个会话下次换模型时一起生效。正在进行的会话，工具列表不变。
- **判断结果钉在策略快照里**（施工 DSR-3）。快照记下判断时的模型引用、当时「联网服务」有没有选这一项、换掉了哪几件工具，所以重新载入会话不会改变工具面。除了换模型，模型这边满不满足条件发生变化时也会重新判断：思考被设成关，或者你在编辑供应商里改了接口或地址。不重新判断的话，请求会直接失败，或者两种搜索都用不了。
- 新会话在第一轮开始时判断，这时还没有发过请求，不影响缓存。

### 4. 驱动接住搜索记录

服务端工具运行的过程接成设计 71 的「供应商侧工具」块，这样它会写进会话日志、在界面上显示成工具步骤、下一轮原样带回。

- 返回的 `output` 里每一次服务端操作是一项 `web_search_call`，带 `kind`（例如 `deep_search`）和 `action`（`type`、`queries`、`sources`）。
- 每一项接成一块供应商侧工具：`name` 写 `deepseek_web_search`（读链接的写 `deepseek_read_url`）；参数写 `action` 的原文；结果的文字写来源的标题和网址，一条一行；私有数据存这一项的原文，下一轮照原样放回 `input`。
- 流式的 `response.web_search_call.*` 事件照开始、进行中、完成接成增量。
- 读链接读到的网页正文不交给客户端，而是加密放在返回里。下一轮必须原样带回，网页内容才会留在上下文里，所以私有数据要完整保存、原样回传。
- 中途换到别家模型时，照设计 71 的做法写成一段文字带给新模型，只有搜索关键词和网址，没有加密的网页正文。
- 返回的 `usage.server_tool_use`（`tool_search`、`tool_fetch` 的次数）记进这一次请求的用量。

### 5. 界面

- 搜索步骤画成和 Miyu 自己的搜索一样的工具步骤，带「供应商」标记，例如「搜索 · DeepSeek」。点开能看到搜了哪些关键词、找到哪些网址。
- 选「DeepSeek Responses 接口内置」时弹出那句提示（见第二节第 2 条），不另外检查、不另外显示提示行。

## 三、DeepSeek 官方的默认接口不变

DeepSeek 的档案（`resources/models/profiles.toml`）照旧写 `driver = "openai-chat"`，接口选「自动」时仍然走 OpenAI 兼容的 chat completions，不改。想用「DeepSeek Responses 接口内置」的人，自己在编辑供应商里把接口选成 Responses（项目主人 2026-11-19 定）。

已有的证据：项目主人在配置里手动选了 Responses 接口，11-19 下午那个会话 23 次请求、调用工具 13 次、思考开着，缓存命中率 87.5%，没有报错。带工具时思考回传不对 DeepSeek 会返回 400，没有报错说明 Responses 接口上的思考回传是对的。

要核对的一件：2026-11-19 实测，Responses 接口只有 `deepseek-flash` 真的处理图片（读出了测试图上的字和颜色），`deepseek-v4-pro` 不看图（回答「无法查看这张图片」，图片没有算进输入 token）。确认 Miyu 的模型资料把走 Responses 接口的 `deepseek-v4-pro` 当成不收图，这样发图时会交给看图模型转成文字。

**核对的结果**（2026-11-19，施工 DSR-1）：

- 模型能收什么输入由模型资料决定，和供应商走哪种接口无关。在沙箱里把 DeepSeek 供应商的接口设成 Responses 之后，核心报告 `deepseek-v4-pro` 的输入只有文字，`deepseek-flash` 是文字和图片；出厂的模型目录和当天从 models.dev 取到的目录写的都是这样。所以把图发给 `deepseek-v4-pro` 时，Miyu 会交给看图模型转成文字，不需要修改。

**直接调用接口时发现的三个限制**（2026-11-19，用 DeepSeek 官方接口实测；第 1、3 条每次只发几十个 token，第 2 条的三次请求里有两次真的搜索了）：

1. **普通工具和联网工具一起声明时，必须开着思考。** 请求里同时有 `function` 工具和 `deepseek_web_search`、`deepseek_read_url`，并且 `reasoning.effort` 是 `none` 时，DeepSeek 返回 400：`client tools can only be used together with deepseek_web_search in thinking mode`。`deepseek-flash` 和 `deepseek-v4-pro` 都是这样。不写 `reasoning`，或者 `effort` 是 `low`、`high` 时，请求成功，模型也能照常调用 `function` 工具。Miyu 的请求总是带着自己的工具，所以激活时要保证思考没有关闭。目前 DeepSeek 的模型资料里思考只有 `low`、`high`、`max` 三档，Responses 接口上选不到「关」；只有在配置里手写 `effort = "off"` 才会发出 `none`。
2. **普通工具和联网工具一起声明时，思考强度必须明确写在请求里，搜索才会执行。** 同一个要搜索的问题发三次：请求里有 `function` 工具和两个联网工具、不写 `reasoning` 时，模型只回答「我去搜索一下」就结束了，没有任何搜索；加上 `"reasoning": {"effort": "high"}` 以后，同样的请求里模型搜索了 2 次，`usage.server_tool_use` 是 `{"tool_search": 2}`；只声明联网工具、不写 `reasoning` 时也会搜索。所以激活时如果没有定思考强度，请求里要明确写上一档（施工 DSR-3 写 `high`，数据在供应商档案的 `search.effort`）。在 Miyu 里实测也是这样：不写时 AI 回答「我没有自带的联网搜索工具」，写了以后一次请求就搜索并答出了当天的版本号和官方公告的网址。

3. **声明了联网工具时，`input` 不能以 assistant 的内容结尾。** 这时最后一项是 assistant 的 `message`、`reasoning` 或 `web_search_call`，DeepSeek 返回 400：`continuing a reply is not supported on the Responses API`。没有声明联网工具时，以 assistant 的消息结尾的请求会成功，模型接着半截往下写。Miyu 的 `openai-responses` 驱动在接着写被打断的回复时，最后一项本来就是带提示的 user 消息，不以 assistant 的内容结尾，所以不受影响。

## 四、有哪些限制

- 是否搜索由模型自己决定。DeepSeek 不允许强制调用这两个工具，指定了会返回 400。
- 深度思考搜索时，模型最多调用约 20 次工具再作答。
- `deepseek_read_url` 只读最后一条用户消息里的链接，最多前 10 个不同的网址。AI 想读其他网址时读不到，这一点比 Miyu 自己的抓取网页弱。
- 搜到的网页内容会放进上下文，每次搜索多用约一万个 token。
- 只有 DeepSeek 官方地址、Responses 接口生效；DeepSeek 默认走 OpenAI 兼容接口，要用户自己改成 Responses。

## 五、施工分几步

1. **核对**：第三节那件（v4-pro 走 Responses 时当成不收图）。
2. **设置**：`net.search.source` 加 `deepseek-responses`，字表中英日三种语言。
3. **组装请求**：激活条件判断、工具面替换（换模型时重新判断，改设置时下个会话生效），缓存守卫测试（同一会话两次请求的 `tools` 数组逐字节相同）。
4. **驱动**：`openai-responses` 驱动接住 `web_search_call`、私有数据原样带回、用量记次数。
5. **界面**：终端和网页「联网服务」的新选项；选它时弹出那句提示。
6. **实测**：用 DeepSeek 官方密钥在沙箱里跑一整轮：搜索、读链接、下一轮接着问刚才的网页、换到别家模型再问。

## 六、决定

- 项目主人 2026-11-19 定：做成「联网服务」里的一个选项「DeepSeek Responses 接口内置」，出厂仍是自动；只在 DeepSeek 官方地址、Responses 接口时激活，其余情况自动退回；不另加供应商开关，不做通用的能力声明；群聊这类场所会话不另做保护；DeepSeek 官方的默认接口不改，仍走 OpenAI 兼容接口；选这个搜索服务时直接弹一句提示「这个搜索服务只适用于 DeepSeek 官方的 Responses 接口」，不检查设置。
- 没选：给所有供应商做通用的「等价服务端工具」能力声明（太复杂，先只给 DeepSeek 做）；无论预设开不开都加工具（预设会管不住、会悄悄多花钱）；在网络搜索包或 DeepSeek 工具包里做特判（层次不对）。
- 重新讨论的条件：其他供应商也要接服务端搜索时，再考虑做成通用机制。
