## 施工单 TLS-1：对方的证书不被信任时，当场报错，不重试

状态：做完，等验收（分支 `step/TLS-1`）。

### 为什么

LiteMiyu 实测发现：模型供应商的证书不被信任时，程序把它当成暂时的错误反复重试，大约 30 秒之后才报错。在 Miyu 上核实，问题同样存在。

实测（2026-11-19，main 的调试版核心，数据根和家目录都在沙箱里）：在本机启动一个使用自签名证书的 TLS 服务，把一家供应商的地址指向它，然后运行 `miyu ask "hi"`。

- `miyu ask` 用了 31 秒才失败，输出 `miyu: 临时错误：error sending request: client error (Connect): invalid peer certificate: Other(OtherError(CaUsedAsEndEntity))`。
- 服务端一共看到 6 次握手，每次都因为客户端拒绝证书而失败，间隔依次是 1、2、4、8、16 秒。

原因：`crates/miyu-http/src/send.rs` 把所有没有 HTTP 状态码的失败交给驱动分类，而驱动把没有状态码的失败一律分成 `retryable`。证书校验失败属于这一类，所以内核按可重试的错误重新发送了五次。但是证书不被信任时，重新连接的结果不会改变，等待的 30 秒没有意义，报出来的「临时错误」也会误导你。

核心主会话决定修（2026-11-19）。

### 改成什么样

发请求失败时，如果错误的来由里有 rustls 的 `InvalidCertificate`（签发者不被信任、证书过期、主机名不符都属于这一种），`send` 直接返回分类 `other` 的错误，内核不重试。给人看的原话是 `证书不被信任：<reqwest 的错连同来由>`，不带地址。

连接被拒绝、DNS 失败等其他失败的处理保持不变，仍然交给驱动分类并重试，因为它们可能是暂时的。

蓝图 `http.md`「出错」的表增加这一行。

### 不做什么

- 不修改抓取网页、列模型等其他请求的路径：它们本来就不重试。
- 不提供「忽略证书错误」的开关。

### 验收

- 修改之前测试是红的。
- 门禁、Windows 目标的 clippy、三个平台的 CI 全部通过。
- 实测：按上面的方法再运行一遍，`miyu ask` 应该立即失败，服务端只看到一次握手。

### 做下来的

- 改前红：`miyu-http` 的 `untrusted::an_untrusted_certificate_is_other_and_says_so` 在修改之前的代码上失败，得到的分类是 `Retryable`，原话是 `… invalid peer certificate: UnknownIssuer`。测试在本机启动一个只做 TLS 握手的服务，证书是放在 `tests/support/` 下的自签名测试证书（`self-signed.cert.der` 和 `self-signed.key.der`，只供这条测试使用）。
- 第一版修改没有生效：rustls 的错误被包在两层 `io::Error` 里，而 `io::Error::source` 会跳过它包着的那个错误本身，第一版只进入了一层。把错误链逐层输出确认结构之后，改成遇到 `io::Error` 就进入它包着的错误。
- 修改之后，`miyu-http` 的测试全部通过（集成测试 39 条），这个 crate 的 clippy 通过。
- 实测（2026-11-19，数据根和家目录都在沙箱里，用这一步编出来的程序，没有使用模型）：
  - 证书不被信任：`miyu ask "hi"` 在 0.84 秒内失败，输出 `miyu: 模型错误：证书不被信任：error sending request: client error (Connect): invalid peer certificate: Other(OtherError(CaUsedAsEndEntity))`，服务端只看到一次握手。修改之前是 31 秒、6 次握手、`临时错误`。
  - 连接被拒绝（地址指向没有服务的端口）：`miyu ask "hi"` 仍然用了 31 秒，输出 `miyu: 临时错误：… tcp connect error: Connection refused (os error 111)`，说明其他失败仍然重试。

### 怎么实测验收

1. 生成一张自签名证书，并用它启动一个 TLS 服务，例如：

   ```bash
   openssl req -x509 -newkey rsa:2048 -nodes -keyout key.pem -out cert.pem -days 2 -subj "/CN=127.0.0.1" -addext "subjectAltName=IP:127.0.0.1"
   openssl s_server -accept 8443 -cert cert.pem -key key.pem
   ```

2. 把 `MIYU_HOME` 指到一个试验用的空目录，运行 `miyu core start`，然后在 `etc/config.toml` 里写入：

   ```toml
   [providers.selfsigned]
   driver = "openai-chat"
   base_url = "https://127.0.0.1:8443/v1"
   local = true

   [providers.selfsigned.models."m1"]
   window = 32000

   [models]
   chat = "selfsigned/m1"
   ```

3. 运行 `miyu ask "hi"`。命令应该在一两秒内失败，输出以 `miyu: 模型错误：证书不被信任：` 开头；`openssl s_server` 只输出一次握手失败。修改之前这条命令要等 31 秒，输出的是 `临时错误`。
