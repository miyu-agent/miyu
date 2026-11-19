## 施工单 DSR-1：核对 `deepseek-v4-pro` 走 Responses 接口时当成不收图

状态：做完，等验收（分支 `step/DSR-1`）。设计 `72-DeepSeek内置联网工具.md` 第五节第 1 步。

### 为什么

设计 72 要在 DeepSeek 官方的 Responses 接口上使用服务端的联网工具。Responses 接口上只有 `deepseek-flash` 会处理图片，`deepseek-v4-pro` 不看图。开始施工之前，需要确认 Miyu 把走 Responses 接口的 `deepseek-v4-pro` 当成不收图的模型，这样发图时会交给看图模型转成文字。

### 做了什么

这一步没有修改代码，只把核对和实测的结果写进设计 72 第三节。

- **核对**：把沙箱里 DeepSeek 供应商的接口设成 `openai-responses`，用协议的 `model.list` 查看模型资料。`deepseek-v4-pro` 的 `inputs` 是 `["text"]`，来源是模型目录；`deepseek-flash` 是 `["text", "image"]`。模型能收什么输入由模型资料决定（蓝图 `models.md`「模型的资料」），和接口无关，所以不需要修改。
- **实测中发现的两个限制**（直接调用 DeepSeek 官方接口，13 次请求，每次几十到一千多个 token）：
  1. `function` 工具和两个联网工具一起声明时，思考关闭（`reasoning.effort` 是 `none`）会返回 400；思考开着时成功，模型能照常调用 `function` 工具。
  2. 声明了联网工具时，`input` 以 assistant 的内容结尾会返回 400；没有声明联网工具时可以，模型会接着写。
  这两条会影响第 3 步（激活判断）和第 4 步（驱动），详细的原话记在设计 72 第三节。

设计 72 原来的第 1 步还包括 chat 和 Responses 两种接口的缓存命中对比、续写的实测。项目主人 2026-11-19 决定 DeepSeek 的默认接口不改，这两项前置实测随之取消。取消之前 chat 接口的那一遍已经运行过（四轮带工具的对话，缓存命中 65%、96%、96%、97%），Responses 的那一遍没有运行。

### 不做什么

- 不修改 DeepSeek 的档案和默认接口。

### 验收

- 门禁通过（这一步只修改文档）。

### 怎么实测验收

1. 把 `MIYU_HOME` 指到一个试验用的空目录，运行 `miyu core start`，在 `etc/config.toml` 里写入：

   ```toml
   [providers.deepseek]
   key = { env = "DEEPSEEK_API_KEY" }
   driver = "openai-responses"
   ```

2. 打开设置页的模型列表，或者用协议的 `model.list` 查看：`deepseek-v4-pro` 的输入类型只有 `文字`，`deepseek-flash` 有 `文字` 和 `图片`。这一步不需要真的密钥。
