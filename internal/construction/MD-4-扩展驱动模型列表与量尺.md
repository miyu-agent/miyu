## MD-4 扩展提供的模型驱动：模型列表和量尺

状态：施工完，待 core1 审（2026-11-07）。设计 `66-扩展提供模型驱动.md` 第四节第 4 条、第六节、第十一节、第十二节第 7 条，第十三节第 4 条。
MD 线的最后一步。

### 定了的

1. **照设计**：扩展可选实现 `driver.models`，主人的模型列表里就有它的模型；`miyu-perf` 加两项（扩展驱动比内置驱动多的首字时延
   p95 5 ms、流式时核心多用的 CPU 一秒的 1%）；文档：协议参考、作者文档「写一个模型驱动」、蓝图、设计 05、15、23。
2. **列表用哪几格（A'，core1 11-07 照推荐定）**：`id`、`window`、`max_output`、`tools` 照「手写 > 供应商的列表 > 目录」接上；`vision` 是
   `true` 的并成 `inputs = ["text", "image"]`，排在手写之后、目录之前；`reasoning` 照收不用（资料里要的是档位表，开关推不出档位），
   参考写明「暂不用」。头一回我说「`tools` 在资料里没有这一格」是错的，核实以后更正，才改推荐成 A'。
3. **量尺怎么跑（A，core1 11-07 照推荐定）**：`miyu-perf` 自己当假扩展装进沙盒数据根（照工具一步的守卫），两项进 `budget.rs`、CI 的
   perf 那一项拦，设计 23 第二节加两行；CI 上连着偶发红的再退到只出报告，退的时候把量到的数写进这张施工单。CPU 那一项写成
   「10 ms（一秒的 1%）」：预算表只认毫秒、兆字节。

### 做了什么

- **端口**（`miyu-session/src/route/driver_port.rs`）：`DriverPort::ask`，只要回答、不交增量（默认实现当连不上，测试的假端口用不着）；
  `miyu-endpoint/src/models/drivers.rs` 实现它，找连接（`link`）、归回应（`broken`）和 `call` 共用。
- **列模型**（`route/extension/listing.rs`）：拉这一家的列表时驱动是扩展的，发 `driver.models {"driver", "key"?, "base_url"?}`，整个 30 秒，
  读 `{"models": [...]}`；多了格、不是对象、写法不对的整份不要。key、地址照挑候选时的规矩交（`extension::reach`，从挑候选那里抽出来
  共用）。`route/lists.rs` 照它分岔。
- **资料**（`miyu-models`）：`ListedModel` 多 `max_output`、`tools`、`vision`（以前的文件没有，照读）；`facts.rs` 照 A' 接上。
- **测试扩展**：`driver:<怎么答>+<怎么编>+<怎么列>`，列法 `list`（两个模型、一个带窗口），别的回「没有这个方法」；`driver.` 开头的方法
  都走 `drivers.rs`（主文件到了行数上限）。
- **量尺**（`miyu-perf/src/measure/driver.rs`、`driver/extension.rs`）：装上 `perf-driver`、两个窗口大的模型（`perf/big`、`perfx/big`），
  量尺自己带参数 `driver-extension` 当驱动；两个会话各垫四句约 250 KB 的话（协议一行最长 1 MiB），之后每次请求约 1 MB，各说
  `--streams` 轮比首字时延的 p95；再各说 `--streams` 轮，内置的照假服务切成 `--pieces` 块、扩展的照记号 `perf-pace` 在一秒里匀着交
  300 段，比一轮核心花的 CPU 的中位数。`rpc.rs` 的 `first_delta` 等第一段正文的增量；原始数据多一格 `driver`（`raw.rs`、`load.rs`），
  对照表多两行（`rows.rs`）。
- **文档**：协议参考 `extensions.md`（中英）`driver.models` 一节和方法表一行；作者文档 `extensions.md`（中英）加「写一个模型驱动」；蓝图
  `models.md`（资料那张表、扩展驱动的列表）、`extensions.md`「扩展提供的模型驱动」第 8 条、`perf.md`（第 5 步、在哪）；设计 05 第七节
  `transport` 那一格、23 第二节两行、66 第四节第 4 条和第六节写明怎么做、谁定的。

### 测试

- `miyu-session` `route/extension/listing/tests.rs`：名字、窗口、最大输出、工具、看图照收；不是对象、少了名字、多了格、类型不对、多了顶层格
  的整份不要。
- `miyu-models` `facts/tests.rs`：列表报的最大输出、工具、看图照列表算、来源是这一份列表，看不了图的不改输入种类，手写的压过列表。
- `miyu-endpoint` `tests/extension_driver_models.rs`（真核心、真扩展进程）：`model.list` 带 `refresh` 经扩展列出两个模型，`listed` 里有
  `provider`，报了窗口的照它、来源是 `provider`；没实现的不出错、列表里没有。
- `miyu-perf` `measure/driver/tests.rs`：认得出这个会话的正文增量；`report/tests.rs`：对照表多的两行照两边一减比（超了的「超」、没超的「过」），
  原始数据写出去读回来一字不差。

### 手搓的变异

10 个，全红：拉列表不分岔到扩展；数组当回答收；最大输出不交；资料不看列表的最大输出、工具，看图的判反；对照表不减内置的那一边
（首字、CPU 各一个）；什么增量都当正文；端口的 `ask` 一律当断了。头一轮「什么增量都当正文」没红：那份单元测试的文件写成了空的
（一个字节），测试一条都没编进去；重写以后红。另外补了一种坏回答 `[[]]`：serde 会把它读成只写了 `models` 的结构，原来那份测试测不出
「回答一定是对象」那一条。

### 不做的

- `reasoning` 接进资料：要扩展报档位表，另起设计。
- `provider.test`、远程的 embedding 走扩展驱动：设计没写。

### 坑

- 流式的 CPU 两边都要匀着交：头一趟 CI 上内置的假服务一次写完、扩展的在一秒里匀着交，量到扩展多 18.0 ms（超了 10 ms）。多出来的
  大半是匀着来的每一段都叫醒核心一次，不是扩展这一跳；假服务加了 `Fake::pace`，两边都照「一秒 300 段」比。

- 量尺的请求垫到 1 MB，沙盒的模型窗口是 20 万，会到压缩线：这两项另配窗口 400 万的模型。
