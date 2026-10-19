手造的语义小模型（施工 S-7 上从 `miyu-embed` 拷过来）：21 个词的词表、4 维，`miyu-embed` 仓库的 `tests/fixtures/make.py` 造的。
核心的测试照它摆一份「内置语义模型」的包目录，拉起测试用的小程序 `miyu-test-embed`（手算这个模型，不编 ONNX Runtime）。
`expected.json` 是 ONNX Runtime 对几组编号算出来的向量，`miyu-test-embed` 的手算照它比。
