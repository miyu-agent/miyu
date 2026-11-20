## 施工单 DSR-2：「联网服务」加「DeepSeek Responses 接口内置」选项

状态：做完，等验收（分支 `step/DSR-2`）。设计 `72-DeepSeek内置联网工具.md` 第五节第 2 步。

### 为什么

设计 72 把 DeepSeek Responses 接口内置的联网工具做成「联网服务」里的一个选项。这一步先加上选项和它的文字；选项什么时候真的生效（激活判断、替换工具面、驱动接住搜索记录）在后面几步做。

### 改成什么样

- `net.search.source` 多一个选项 `deepseek-responses`，排在 `deepseek` 后面。它不是 `search.json` 里的一家搜索服务：Miyu 自己的 `web_search`、`web_fetch` 碰到它时使用自动的顺序，不需要密钥和地址。
- 字表（中文、英文、日文）：
  - 选项 `deepseek` 的名字从「DeepSeek」改成「DeepSeek 搜索接口」，和新选项区分开。
  - 新选项的名字是「DeepSeek Responses 接口内置」。
  - `deepseek` 和 `deepseek-responses` 两个选项各有一句说明（`option_descriptions`）。
  - 新选项有一句选中时弹出的提示（`option_notices`）：「这个搜索服务只适用于 DeepSeek 官方的 Responses 接口」。
- 字表的结构多一格 `option_notices`（选项到一句话）。`miyu_config::words::check` 检查它：提示必须写给这一项存在的选项，不能是空的。
- 协议：`config.schema` 里的选项多一格可选的 `notice`。界面在你选中带 `notice` 的选项时弹出这一句，只有一个「知道了」按钮，选项仍然保存。弹窗由界面实现，不在这一步。

蓝图 `config.md`（配置项的表、「给人看的字」的表、协议）、`tools/web_search.md`，协议参考 `config.md` 的中英文两页一起修改；配置的样本重新生成。

### 不做什么

- 不做激活判断和工具面替换（第 3 步）。这一步之后选中新选项，效果和「自动」相同。
- 不修改手册：功能在第 6 步实测之后再写进手册。

### 验收

- 修改之前测试是红的。
- 门禁、三个平台的 CI 全部通过。

### 做下来的

- 改前红：新加的四条测试在修改之前的代码上无法编译，因为它们用到的选项值和字表的 `option_notices` 一格都还不存在。四条测试是：`miyu-net` 的 `settings::tests::the_deepseek_responses_option_is_listed_and_is_not_a_shipped_source` 和 `search_keys::deepseek_responses_searches_in_the_automatic_order`，`miyu-config` 的 `words::tests::an_option_notice_must_belong_to_an_option_and_say_something`，`miyu-core` 的 `search_source::the_schema_lists_the_option_with_its_notice_and_a_set_takes_it`。
- 配置的四份样本用 `MIYU_SAMPLE_WRITE=1` 重新生成，蓝图 `config.md` 里嵌的参考文件样本块一起更新。
- 实测（2026-11-19，数据根和家目录都在沙箱里，用这一步编出来的程序，没有使用模型）：
  - `miyu config set net.search.source deepseek-responses` 输出 `已设置 net.search.source = "deepseek-responses"（下一轮生效）`；`miyu check` 输出 `没有问题`。
  - 用协议的 `config.schema` 查看这一项（中文连接）：选项依次是 `auto`、`exa`、`anysearch`、`deepseek`、`deepseek-responses`、`tavily`、`brave`、`searxng`。`deepseek` 的名字是 `DeepSeek 搜索接口`，带说明，没有配置密钥时后面有 `需要密钥`；`deepseek-responses` 的名字是 `DeepSeek Responses 接口内置`，带说明和 `notice`（`这个搜索服务只适用于 DeepSeek 官方的 Responses 接口`），后面没有 `需要密钥`。
  - 没有实测界面：弹窗由界面实现，这一步只有协议和文字。

### 怎么实测验收

1. 运行 `miyu core stop`，确认之后的命令由新版本的核心回答。
2. 运行 `miyu config set net.search.source deepseek-responses`，应该输出 `已设置 net.search.source = "deepseek-responses"（下一轮生效）`。
3. 运行 `miyu config explain net.search.source`，说明下面的第一行应该是 `"deepseek-responses"  系统配置  …  ← 生效`；说明里提到的是 `「DeepSeek 搜索接口」`。
4. 界面跟上之后：在设置页的 `联网服务` 下拉里选择 `DeepSeek Responses 接口内置`，会弹出 `这个搜索服务只适用于 DeepSeek 官方的 Responses 接口`，按 `知道了` 之后选项保存。
