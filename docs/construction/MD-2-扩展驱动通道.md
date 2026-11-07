## MD-2 扩展提供的模型驱动：通道和执行器的分岔

状态：施工完，待 core1 审（2026-11-07）。设计 `66-扩展提供模型驱动.md` 第三、五、八、九节，第十三节第 2 条。接在 MD-1（登记和选用）后面：MD-1 起扩展
驱动一律当用不了，这一步接上通道，真发给扩展。缓存的查和记（`driver.encode`、`driver-check`、`sent` 对不上的 `WARN`）是 MD-3，模型列表和
量尺是 MD-4。

### 定了的

1. **照设计**：`driver.call`（核心发、要回答）、`driver.delta`（扩展发的通知）、`driver.cancel`（核心发的通知），走扩展现成的那条标准
   输入输出连接；执行器在 `route/exchange.rs` 分岔，交给内核的还是同一串 `Delta` 和同一个结尾；出错的分类、重试、冷却、换池照第八节，
   都在核心；`model.called` 记 `sent`；key 只交这一家配置里引用的那一把。
2. **端口**（技术细节，按推荐定）：执行器在 `miyu-session`，扩展在上一层的 `miyu-endpoint`。照「下架」那个端口（`route/retire.rs`）的做法，
   `miyu-session` 定一个 `DriverPort`，核心起来以后装进模型资料（`ModelData::on_drivers`）；没装的（测试、第一次接入）扩展驱动照旧当用不了。
   端口只管「在不在、声明了什么」和「把这一次送过去、把增量和结尾送回来」；参数怎么编、增量怎么查、超时、出错归类都在执行器里。
3. **驱动不在**（第八节第 5 条，落到哪一格照 `route/ended.rs` 定）：
   - 没装、驱动名不在清单里、包关着、没批 `model.driver`、停下了（`stopped`）：挑候选时就当用不了（`no_model`，原话
     `model driver "<包>/<名>" is unavailable: <原因>`），单个模型当场报，池里照现成的规矩跳过、换下一个；不重试。
   - 正在退避等重新拉起（`waiting`）、刚拉起还没握手（`starting`）：发的时候当 `Retryable`，`waiting` 的 `retry_after_ms` 照它的 `retry_in`。
4. **key 和地址**（第九节）：`key = "none"` 的不取、不交；`optional` 的写了才取，写了取不到照内置驱动当 `no_model`；`required` 的没写当
   `no_model`（`miyu check` 已经报 `driver_key_missing`）。`base_url` 是这一家写的；扩展驱动的供应商可以不写地址（自己登录、走订阅的
   用不着），不写的不交。`driver.call` 的参数不进日志，只记一行哪个包、哪个驱动、第几次调用。
5. **请求**：`request` 是规范字节原样（同 `canonical_bytes()`，不重新解析、不重新编），`output_cap`、`described` 已经在里面（有的才写）。
   别家驱动的私有数据（第四节第 3 条）拿掉再交：有别家私有数据的请求拷一份把那几格去掉，没有的原样交。扩展驱动的家族是
   `<包编号>/<驱动名>`：内核的驱动家族这一种写法多收一个 `/`（只收「名字/名字」，以前的数据都没有 `/`，读得进）。
6. **图**：声明了 `images = true` 的，请求里每张图照 `{哈希: {"media_type", "data"}}`（base64）交进 `blobs`；没声明的不交，扩展照
   `described` 用转述。文件（PDF 这些）第一版不交。
7. **增量**：照内核的累加器在执行器里先查一遍（块照先后开、开了才有字、收了不再写）；不对的发 `driver.cancel`，这一次当 `unclassified`
   结束，原话 `the driver sent a bad delta: <哪里不对>`，记一行 `WARN`。不认识的 `call` 丢掉。
8. **时限**：同内置驱动走 HTTP 的空闲时限（照思考强度放大过的 `Ready.idle`）和总时限（`idle × TOTAL_FACTOR`）：发出去到第一条增量、
   两条增量之间、最后一条增量到结尾都不能超过空闲时限，整个不超过总时限；超了的发 `driver.cancel`，当 `retryable`，原话
   `the driver went quiet`。
9. **断了**：连接断了（扩展崩了、退出了），在等的当 `retryable`，原话 `the driver went away`。
10. **出错的分类**：`error.class` 是 `retryable`、`rate_limited`、`context_too_long`、`auth`、`quota`、`content_policy`、`unclassified`
    之一，不认识的当 `unclassified`；`retry_after_ms` 只认 `rate_limited` 的；`message` 截到 500 个字。回答本身写坏了的当 `unclassified`。

### 做了什么

- **内核**（`miyu-kernel`）：`model.called` 多一格 `sent`（真发出去的字节的哈希，扩展驱动报的；以前的日志没有）；`ModelEnded`、`AsideEnded`
  带 `sent`，主请求经 `Ending::Said`、旁路请求先记到在路上的那一次（`Aside`）再收尾；驱动家族多收 `<名字>/<名字>`（`check_family`）；
  指纹交出三样哈希（`Fingerprint::hashes`）。
- **端口**（`miyu-session/src/route/driver_port.rs`）：`DriverPort`（`present`、`call`）、`Declared`、`NotYet`、`Broken`、`Pending`（对外叫
  `DriverPending`，和 `spawn::Pending` 不撞）；模型资料 `ModelData::on_drivers`（只装一次）。
- **执行器**（`route/extension.rs`、`extension/wire.rs`）：`Choice` 多一格 `extension`（看不看图、key、地址：`Reach`），`Ready` 的驱动改成
  `Speaker`（内置的 / 扩展的）；`exchange` 开头分岔；参数照规范字节拼（不重新解析）；增量照累加器先查；时限、断了、被拒、写坏了照「定了的」
  归类；`Exchanged::Ended` 多 `wire`，`send.rs` 交给回报（`Reports::wired`），一次性入口不写 `model.called`、不管它。
- **模型资料**（`miyu-models`）：手写了扩展驱动的供应商可以不写地址，记成空的字。
- **核心这一头**（`miyu-endpoint/src/models/drivers.rs`、`reverse.rs`、`connection.rs`）：`Drivers` 实现端口，查装没装、驱动名、批没批、开关、
  停没停；找握了手的连接，退避中的报还要等多久；`Peer::call_raw`（参数原文拼进那一行）、`stream`/`unstream`/`delta`（照 `call` 分增量）；
  读循环把 `driver.delta` 交给 `Peer`；丢下没答完的发 `driver.cancel`；能调什么的表加 `driver.call`、`driver.cancel`。`miyu-core` 起来时
  `start_drivers`。
- **测试扩展**：`driver:<怎么答>`（`echo`、`half`、`pair`，`miyu-test-extension/drivers.rs`）。
- **顺手**（core1 审 MD-1 时提的）：`driver_ref::with_installed` 每装卸一次多留一份选项表，改成照内容复用已经留着的（`kept`）。
- **文档**：协议参考 `extensions.md`（中英）加 `driver.call`、`driver.delta`、`driver.cancel` 一节和方法表一行；作者文档 `manifest`（中英）指过去；
  蓝图 `extensions.md`（「扩展提供的模型驱动」五条、在哪、守着它的）、`models.md`（第四条第 2 条）、`kernel/events-bodies.md`（`sent`）、
  `log.md`（`driver bad delta`、`driver bad answer`）；设计 66 第九节第 1 条（地址可以不写）。

### 测试

- `miyu-session` `route/extension/tests.rs`（假端口直接走 `exchange`）：正常一段（增量、用量、`sent`、参数的样子：`driver`、`key`、不交
  `base_url` 和 `blobs`、`params`、规范字节原样、指纹是 16 位十六进制）；断了、被拒、乱序、回答写坏了、不认识的增量各归各类；增量不对的
  不交给内核、发 `driver.cancel`；没动静的超时、一段一段来的不超时（每来一段重算）、一直来也有总时限；被叫停；连不上的带等多久、没发；
  别家的私有数据不交、自己家的照交；私有数据记成扩展的家族、`truncated`。
- `extension/wire/tests.rs`：参数里请求排最前、没有的不写；每种增量读得出、写错的拒；七种分类、不认识的当 `unclassified`、等多久只认
  限速的、原话截到 500 个字、什么都不报也行、写坏的回答（含 `[]`）拒；图去重、带类型。
- `miyu-session` `tests/route_extension_driver.rs`：没装通道的绝不退回内置驱动（MD-1 的两条）；装了假端口：回复是扩展交的字，每一条
  `model.called`（主请求、起标题）记这一家和 `sent`，只交这一家的 key、别家的 key 一次都不出现；`key = "none"` 写了也不交、`required`
  没写当 `no_model`；驱动不在的当场 `no_model`（原因写明），池里换下一个，一次都不调。
- `miyu-endpoint` `tests/extension_driver_call.rs`（真核心、真扩展进程）：`model.call` 的回答是扩展的字、用量对；记下的 `driver.call`
  只带这一家的 key、不带地址，别家的 key 不出现，假服务器一个请求都没收到；两次同时调、扩展交错着交，各拿到自己的；交了一半崩了的出错
  （`the driver went away`）；没批的不用、原因写明。`src/models/drivers/tests.rs`：没答完就丢下的发 `driver.cancel`、答完的不发，丢下以后
  的增量不再收。
- `miyu-kernel` `session/tests/title.rs`：起标题这类旁路请求结尾带的 `sent` 记进 `model.called`。`id/tests.rs`：家族收 `<包>/<驱动名>`，多一个 `/`、空的一段、大写的拒。`miyu-models`：扩展驱动不写地址也认、内置的照旧要。
  `miyu-config` `driver_ref/tests.rs`：同样的列表用的是同一份选项表。

### 手搓的变异

18 个，全红：`none` 的照样交 key、`required` 没写照样发；别家的私有数据照交；空闲时限不重算、没有总时限；收尾前不读空增量；不查增量；
`quota` 当成 `auth`、哪一类都带等多久、原话不截；`sent` 不交给回报、旁路请求的 `sent` 丢了；家族收好几个 `/`；扩展驱动也要地址；不查批没批；
丢下不发 `driver.cancel`；增量交给别的调用；选项表不复用。头一轮「旁路请求的 `sent` 丢了」没红（集成测试里多出来的那一次不是写
`model.called` 的旁路请求），补了内核里起标题那一条以后红；「丢下不发 `driver.cancel`」头一轮照格式化前的原文找不到、没跑，照现在的
写法重跑红。

### 不做的

- `driver.encode`、`miyu dev driver-check`、`sent` 对不上的 `WARN`：MD-3。
- `driver.models`、`miyu-perf` 的两项、作者文档「写一个模型驱动」：MD-4。
- 文件（PDF 这些）交给扩展：设计没写，第一版不交。

### 坑

- 回答和增量走同一条连接：读的那一头先把增量交出去、再交回答，执行器拿到回答时增量已经都在队列里，收尾前要把队列读空。
