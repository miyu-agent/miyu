"""生成 miyu-embed 的字节级 BPE 测试数据（施工 R-5 六补）。不进门禁，换数据时手动跑一次：

    python make_bpe.py <这个 fixtures 目录> [granite 的 tokenizer.json] [Release 的 model_q4.onnx]

要装 tokenizers 0.23.3、onnx、onnxruntime、numpy。句子和训练用的字都是造的，不是谁的记忆、对话。生成：

1. tiny-bpe/：照 tokenizers 训练的小 BPE（granite r2 那一套配置：o200k 的正则单独切、按字节换字、ignore_merges），另加几个
   整词（照合并切不出它们，测整段先查词表）；再照 ../tools/bpe_files.py 转成 vocab.txt、merges.txt。手造的小模型 model.onnx
   只有 input_ids、attention_mask 两个输入（granite 没有 token_type_ids），清单 manifest.toml，expected.json 是几句的编号和
   小模型照 onnxruntime 算的向量。
   odd-input.onnx 另要一个不认识的输入 position_ids（载入时拒）。
2. bpe-tokens.json：小 BPE 对一批句子的结果（不截断）。
3. granite-tokens.json（给了 granite 的 tokenizer.json 才生成）：真词表对同一批句子的结果，`#[ignore]` 的那一条照它比。
4. real.json（再给了 Release 的 model_q4.onnx 才生成，施工 R-5 七补）：出厂模型对几句的整个向量（归一化的第一格），照 Python
   的 onnxruntime 算，`tests/real.rs` 照余弦比。
"""
import hashlib, json, os, subprocess, sys
import numpy as np, onnx, onnxruntime as ort
from onnx import helper, TensorProto, numpy_helper
from tokenizers import Tokenizer, Regex, models, pre_tokenizers, processors, trainers

OUT = sys.argv[1]
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "..", "tools"))
from bpe_files import SPLIT  # noqa: E402

START, END = "<|startoftext|>", "<|return|>"
SPECIAL = [START, "<|endoftext|>", END]

corpus = [
    "The quick brown fox jumps over the lazy dog.", "She's reading a book; they've gone home, I'm here and we'll see.",
    "Running, jumping and swimming are good exercises.", "HTTPServer handles JSONParser requests on port 8080.",
    "用户喜欢喝绿茶，每天早上八点起床。", "明天下午三点在图书馆见面，记得带上笔记本。", "这只小猫叫团子，今年三岁了。",
    "今日はいい天気ですね。カタカナとひらがなを混ぜて書きます。", "한국어 문장도 조금 넣어 봅니다. 안녕하세요!",
    "Привет, как дела? Это русский текст.", "مرحبا بالعالم، هذا نص عربي قصير.", "Ελληνικά γράμματα και λέξεις.",
    "def main():\n    print('hello, world')\n    return 0\n", "if (a <= b && c != d) { x += 1; }", "price: $19.99, 50% off!!!",
    "IT'S HERE, DON'T GO, WE'RE OK, I'M FINE, YOU'LL SEE, HE'D GO, THEY'VE WON",
    "2026-10-18 08:30:00 1234567890", "emoji 😀🎉👍🏽 and 👨‍👩‍👧 family", "café naïve résumé coöperate",
    "  leading and trailing spaces  ", "tabs\tand\nnewlines\r\nmixed\n\n\nend", "全角，标点！？（括号）【方括号】“引号”",
]
def byte_level(text):
    """GPT-2 的字节到字：可见的照原样，别的挪到 256 以后。"""
    table, n = {}, 0
    for b in range(256):
        if 33 <= b <= 126 or 161 <= b <= 172 or 174 <= b <= 255:
            table[b] = chr(b)
        else:
            table[b] = chr(256 + n)
            n += 1
    return "".join(table[b] for b in text.encode("utf-8"))


words = [byte_level(w) for w in ("unbelievable", " unbelievable", "団子", " HTTPServer")]  # 照合并切不出的整词

tok = Tokenizer(models.BPE(ignore_merges=True))
tok.pre_tokenizer = pre_tokenizers.Sequence([
    pre_tokenizers.Split(Regex(SPLIT), behavior="isolated", invert=False),
    pre_tokenizers.ByteLevel(add_prefix_space=False, use_regex=False),
])
trainer = trainers.BpeTrainer(vocab_size=600, min_frequency=2, show_progress=False,
                              initial_alphabet=pre_tokenizers.ByteLevel.alphabet())
tok.train_from_iterator(corpus * 20, trainer)
trained = json.loads(tok.to_str())
vocab, merges = trained["model"]["vocab"], trained["model"]["merges"]
for word in words:
    vocab.setdefault(word, len(vocab))
tok = Tokenizer(models.BPE(vocab=vocab, merges=[tuple(m) if isinstance(m, list) else tuple(m.split(" ")) for m in merges],
                           ignore_merges=True))
tok.pre_tokenizer = pre_tokenizers.Sequence([
    pre_tokenizers.Split(Regex(SPLIT), behavior="isolated", invert=False),
    pre_tokenizers.ByteLevel(add_prefix_space=False, use_regex=False),
])
tok.add_special_tokens(SPECIAL)
tok.post_processor = processors.TemplateProcessing(
    single=f"{START} $A {END}", special_tokens=[(t, tok.token_to_id(t)) for t in (START, END)])
d = os.path.join(OUT, "tiny-bpe")
os.makedirs(d, exist_ok=True)
scratch = os.path.join(d, "tokenizer.json")
tok.save(scratch)
printed = subprocess.run([sys.executable, "-I", os.path.join(HERE, "..", "..", "tools", "bpe_files.py"), scratch, d],
                         check=True, capture_output=True, text=True).stdout
os.remove(scratch)

texts = [
    "", " ", "   ", "\t", "\n", "\r\n", "a", "I", "the", "The quick brown fox", "THE QUICK", "camelCaseWords",
    "HTTPServer", "unbelievable", " unbelievable", "unbelievably", "she's", "SHE'S", "they've we'll I'm you'd",
    "don't stop", "IT'S DON'T WE'RE", "It'S", "YOU'LL HE'D", "123", "1234567", "3.14159", "v2.0.1", "2026-10-18", "$19.99", "a+b=c", "!!!", "...\n", "a/b/c\n",
    "hello  world", "hello   \nworld", "trailing   ", "  leading", "x \t y", "line1\nline2\r\nline3", "\n\n\n",
    "用户喜欢喝绿茶", "团子", "団子", "今日はいい天気", "カタカナ", "한국어", "Привет мир", "مرحبا", "Ελληνικά",
    "😀", "👍🏽", "👨‍👩‍👧", "é", "café", "naïve", " nbsp ", "全角　空格", "零宽​空格",
    "BOM﻿在中间", "控制\u0001字符\u007f", "\u0085next line", " line sep", "替换�字符",
    "𠀀𪚥 扩展区", "ｆｕｌｌ ｗｉｄｔｈ", "Ⅻ ① ²", "def f(x):\n    return x * 2\n", "if (a <= b) { x += 1; }",
    "a\x00b", "\x0b\x0c控制", "\U000F0000 私用区", "\U00100001", "x" * 300, "中" * 200, "ab " * 100, " " * 50 + "end",
]
rows = [{"text": t, "ids": tok.encode(t).ids} for t in texts]
plain = Tokenizer.from_str(tok.to_str().replace('"ignore_merges":true', '"ignore_merges":false'))
assert plain.to_str() != tok.to_str()
print("ignore_merges changes", sum(plain.encode(t).ids != tok.encode(t).ids for t in texts), "texts")
json.dump(rows, open(os.path.join(OUT, "bpe-tokens.json"), "w"), ensure_ascii=False, indent=0)

# 小模型：只有两个输入。输出 = 每个位置的嵌入，照掩码加上全句的和（第一格随整句变）。
size = tok.get_vocab_size()
rng = np.random.default_rng(11)
E = rng.normal(size=(size, 4)).astype(np.float32).round(3)
ids = helper.make_tensor_value_info("input_ids", TensorProto.INT64, ["batch", "seq"])
mask = helper.make_tensor_value_info("attention_mask", TensorProto.INT64, ["batch", "seq"])
out = helper.make_tensor_value_info("last_hidden_state", TensorProto.FLOAT, ["batch", "seq", 4])
nodes = [
    helper.make_node("Gather", ["E", "input_ids"], ["h"]),
    helper.make_node("Cast", ["attention_mask"], ["mf"], to=TensorProto.FLOAT),
    helper.make_node("Unsqueeze", ["mf", "axis"], ["m"]),
    helper.make_node("Mul", ["h", "m"], ["hm"]),
    helper.make_node("ReduceSum", ["hm", "seqaxis"], ["s"], keepdims=1),
    helper.make_node("Add", ["hm", "s"], ["last_hidden_state"]),
]
inits = [numpy_helper.from_array(E, "E"), numpy_helper.from_array(np.array([-1], dtype=np.int64), "axis"),
         numpy_helper.from_array(np.array([1], dtype=np.int64), "seqaxis")]
m = helper.make_model(helper.make_graph(nodes, "tiny-bpe", [ids, mask], [out], inits),
                      opset_imports=[helper.make_opsetid("", 13)], producer_name="miyu-embed-fixture")
m.ir_version = 8
onnx.checker.check_model(m)
onnx.save(m, os.path.join(d, "model.onnx"))
model = open(os.path.join(d, "model.onnx"), "rb").read()
open(os.path.join(d, "manifest.toml"), "w").write(
    "# 测试用的小 BPE（施工 R-5 六补，`fixtures/make_bpe.py` 造的）：只有两个输入的小模型、4 维。\n"
    'id = "tiny-bpe"\ndims = 4\npooling = "cls"\nmax_tokens = 8\n\n' + printed.split("\n\n", 1)[0] + "\n\n"
    f'[[files]]\nrole = "model"\nname = "model.onnx"\nsha256 = "{hashlib.sha256(model).hexdigest()}"\nsize = {len(model)}\n'
    + "\n" + printed.split("\n\n", 1)[1])
# 要一个不认识的输入（position_ids）的小模型：载入时就拒。
odd = helper.make_tensor_value_info("position_ids", TensorProto.INT64, ["batch", "seq"])
m = helper.make_model(helper.make_graph(nodes, "odd-input", [ids, mask, odd], [out], inits),
                      opset_imports=[helper.make_opsetid("", 13)], producer_name="miyu-embed-fixture")
m.ir_version = 8
onnx.save(m, os.path.join(d, "odd-input.onnx"))
s = ort.InferenceSession(os.path.join(d, "model.onnx"), providers=["CPUExecutionProvider"])
expected = []
for t in ["the quick brown fox", "用户喜欢喝绿茶", "unbelievable", "x" * 300]:
    c = tok.encode(t).ids
    c = c[:7] + [c[-1]] if len(c) > 8 else c
    a = np.array([c], dtype=np.int64)
    v = s.run(None, {"input_ids": a, "attention_mask": np.ones_like(a)})[0][0, 0]
    expected.append({"text": t, "ids": c, "vector": [float(x) for x in (v / np.linalg.norm(v)).astype(np.float32)]})
json.dump(expected, open(os.path.join(d, "expected.json"), "w"), ensure_ascii=False, indent=0)

if len(sys.argv) > 2:
    granite = Tokenizer.from_file(sys.argv[2])
    granite.no_truncation()
    json.dump([{"text": t, "ids": granite.encode(t).ids} for t in texts],
              open(os.path.join(OUT, "granite-tokens.json"), "w"), ensure_ascii=False, indent=0)
if len(sys.argv) > 3:
    real = ort.InferenceSession(sys.argv[3], providers=["CPUExecutionProvider"])
    rows = []
    for t in ["用户养了一只猫，叫团子，三岁", "回答先说结论，再给依据", "rtx 4090 显卡", "喝什么茶",
              "The user's cat is called Tuanzi.", "猫の名前は団子です"]:
        a = np.array([granite.encode(t).ids], dtype=np.int64)
        v = real.run(None, {"input_ids": a, "attention_mask": np.ones_like(a)})[0][0, 0]
        rows.append({"text": t, "vector": [round(float(x), 7) for x in v / np.linalg.norm(v)]})
    json.dump(rows, open(os.path.join(OUT, "real.json"), "w"), ensure_ascii=False)
print("ok", len(texts), "texts,", size, "tokens")
