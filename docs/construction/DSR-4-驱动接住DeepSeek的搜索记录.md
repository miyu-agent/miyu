## 施工单 DSR-4：`openai-responses` 驱动接住 DeepSeek 的搜索记录

状态：做完了，等合并（分支 `step/DSR-4`，接在 `step/DSR-3` 后面）。设计 `72-DeepSeek内置联网工具.md` 第五节第 4 步。

### 为什么

施工 DSR-3 之后，DeepSeek 能在自己的服务器上搜索并回答，但是驱动把回应里的搜索记录（`web_search_call` 项）丢掉了，所以有三个问题：

- 会话日志和界面上看不到 AI 搜索过什么。
- 下一轮请求里没有这些项，AI 不记得自己搜索过。实测里下一轮问它「你刚才是怎么查到的」，它回答「上一条的内容是我编造的」。
- 用量里没有搜索和抓取的次数。

### 改成什么样

- **解码**：回应里每一个 `web_search_call` 项接成设计 71 的一块「供应商侧工具」（`hosted_tool`）。
  - 名字：DeepSeek 的项（带 `kind`）按 `action.type` 取名，打开网页、在网页里查找的写 `deepseek_read_url`，其他写 `deepseek_web_search`；没有 `kind` 的项（OpenAI 自己的）写 `web_search`。
  - 参数：`action` 的原文。
  - 结果：搜索写来源的标题和网址，一条一行；打开网页写网页的网址和标题；失败的项标成失败，写出错误码。
  - 私有数据：不保存。
  - 流式：`response.output_item.added` 时开块，`response.output_item.done` 时收块。
- **带回**：下一次请求里，这些块按设计 71 写成一段文字，放进上一轮的回答里。同一个驱动和换到其他驱动时写法相同，所以编码这一侧不需要新代码。
- **只声明搜索**：供应商档案的替换表改成只把 `web_search` 换成 `deepseek_web_search`。`deepseek_read_url` 不再声明，Miyu 自己的 `web_fetch` 保留在工具面里。
- **用量**：内核的用量多两格可选的 `searches`、`fetches`，记 DeepSeek 报的搜索次数和抓取次数；没有报、报的是 0 时不写，以前的日志字节不变。
- **界面的对照表**：`resources/core/view.json` 的 `hosted` 表加上 `deepseek_web_search` 和 `deepseek_read_url`，界面把它们画成和 Miyu 自己的搜索、抓取网页一样的工具步骤；搜索的标题写 `queries` 的第一句。

蓝图 `drivers/openai-responses.md`、`models.md`（档案的样本）、`kernel/events-bodies.md`、`view.md`，协议参考里说明用量的中英文两页，设计 71 的「没做的」和设计 72 一起修改。

### 和设计 72 原稿不一样的两处

这两处都是 2026-11-19 直接调用 DeepSeek 官方 Responses 接口实测出来的，主会话同意以后修改，设计 72 第二节、第三节和「决定」一节已经跟着改。

1. **不声明 `deepseek_read_url`，`web_fetch` 保留。** 请求里有 `function` 工具，又同时声明两个联网工具，提问里带着网址时，3 次请求都没有执行任何联网工具（输入只有 312 个 token）。只声明 `deepseek_web_search` 时，同一句提问模型用深度搜索打开了那个网址并答对了（输入 3,759 个 token）。施工 DSR-3 的实测里 AI 有时改用 `shell` 运行 `curl`，原因就是这一条。
2. **搜索记录写成文字带回，不保存私有数据。** 原样把 6 项 `web_search_call` 放回 `input` 时，输入只有 1,366 个 token，模型回答自己没有搜索过。写成文字放进上一轮的回答时，输入 3,487 个 token，模型正确列出了搜索词和打开过的网页。

### 不做什么

- 不处理回答正文里的引用标注（`url_citation`）。
- 不改变其他供应商的行为。

### 验收

- 修改之前测试是红的。
- 用真实抓到的 DeepSeek 流式回应当样本：解码出的块、下一次请求里写成的文字、用量的次数都有测试。
- 门禁、三个平台的 CI 全部通过。
- 实测：用 DeepSeek 官方密钥在沙箱里搜索一次，下一轮接着问刚才搜到的内容，给一个网址让 AI 读取，再换到其他模型接着问。

### 做下来的

- `crates/miyu-drivers/src/openai_responses/hosted.rs`（新）：`web_search_call` 的工具名、参数原文、结果的文字。`decode.rs` 在项开始时开块，在项结束时交出结果并收块。
- 编码这一侧没有改动：这些块不带私有数据，`input.rs` 原有的写法把它们写成设计 71 的那段文字。
- `resources/models/profiles.toml`：DeepSeek 的 `search.replaces` 只留 `web_search` 换成 `deepseek_web_search` 这一对。
- 内核的 `Usage` 多 `searches`、`fetches` 两格，`openai_responses/usage.rs` 读 `usage.server_tool_use`。
- `resources/core/view.json` 加两个名字；`miyu-view` 的标题在参数是数组时取第一个元素。
- 样本：`docs/designs/samples/drivers/openai-responses/streams/deepseek-web-search.sse` 是 2026-11-19 录下的真实回应。原来的 `unknown-item.sse` 用 `web_search_call` 当「不认识的项」，现在换成 `image_generation_call`。
- 测试：`crates/miyu-drivers/tests/openai_responses_web_search.rs`。新加的测试在修改之前无法编译（`Usage` 没有那两格）。

### 已知的局限

- **模型不一定使用内置搜索。** 请求里的声明和思考强度都是对的，但是 Miyu 的工具面里还有 `web_fetch` 和 `shell`，模型经常自己抓网页。实测 10 个需要联网的提问，只有 3 次用了内置搜索，1 次只说「我去搜索」就结束了，其余用 `web_fetch` 或 `curl` 答出来了。把抓到的两份真实请求直接发给 DeepSeek 做对照：原样发送是 0/5、0/5；去掉 `web_fetch` 是 0/3、0/3；系统提示词末尾加两句说明（内置搜索在供应商那边运行、不在函数列表里，`web_fetch` 只读一个已知的网址）是 2/3、3/3。要不要加这两句需要项目主人决定，已经交给主会话，同意的话另开一步做。
- 深度搜索打开的网页，内容不交给 Miyu。下一轮带回的文字里只有网址和标题，所以追问网页的细节时，AI 需要重新搜索或者用 `web_fetch` 读取。实测里 AI 在追问时自己说明了这一点。

### 怎么实测验收

2026-11-19 在沙箱里实测（`MIYU_HOME` 和 `HOME` 都指向临时目录，DeepSeek 官方密钥只放在环境变量里，沙箱的文件里搜不到密钥原文）。配置：DeepSeek 供应商的接口设成 `openai-responses`，对话模型 `deepseek/deepseek-flash`，「联网服务」设成 `deepseek-responses`；另配一个走 OpenAI 兼容接口的 DeepSeek 供应商，用来测试换模型。

1. 提问「网上最近对 niri 窗口管理器的评价怎么样？简单总结两句并给出来源。」AI 一次请求里完成了搜索并给出带网址的回答。会话日志里这条回复有 1 块 `deepseek_web_search` 和 11 块 `deepseek_read_url`，都没有私有数据；这一次请求的用量里 `searches` 是 4。工具面是 23 件，有 `web_fetch`，没有 `web_search`。
2. 接着问「不要再搜索，也不要运行命令。你上一轮搜索用了哪些关键词？打开过哪些网址？各列两个。」AI 答出了 `niri window manager review 2026` 和 `https://itsfoss.com/niri-window-manager/`，和日志里的记录一致。这一轮的输入是 8,107 个 token。
3. 接着问「读一下 https://blog.rust-lang.org/ 这个网页，最新一篇文章的标题和日期是什么？」AI 答对了（2026 年 11 月 16 日的《Enabling the parallel frontend on nightly》）。这一次它用内置搜索打开了网页，用量里 `fetches` 是 1；另一次同样的提问里它用的是 `web_fetch`，也答对了。
4. 换到走 OpenAI 兼容接口的 `deepseek-flash` 再问「第一轮你搜索用的第一个关键词是什么？」AI 答出 `niri window manager review 2026`。换模型以后工具面恢复成 24 件，`web_search` 回来了。

自己验收时可以这样做：按上面的配置启动核心，问一个需要搜索的问题，再追问它搜索了什么。如果第一轮 AI 没有使用内置搜索（见「已知的局限」），换一个新会话再问一次。
