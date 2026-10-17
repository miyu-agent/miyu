"""把 Hugging Face 的 tokenizer.json 转成 miyu-embed 的字节级 BPE 用的两个文件（施工 R-5 六补，`docs/blueprint/recall.md` 第四条第 5 款）：

    python -I bpe_files.py <tokenizer.json> <输出目录>

只用标准库。写出 vocab.txt（一行一个词，行号是编号，特殊的词接在后面）、merges.txt（一行一条合并「左 右」，行号是先后），
印出清单里 [tokenizer] 那一段和两个文件的 [[files]]。miyu-embed 只认一种配置：不规范化；照 GPT-4o（o200k）那条正则单独切出
每一段，再按字节换成字（不加前导空格、不再切）；BPE 先整段查词表（ignore_merges）、不退到字节编号、不带前后缀；前后各加一个特殊
的词。不是这一种的拒，说清哪里不一样：照着算会悄悄算错。
"""
import hashlib, json, os, sys

# 和 crates/miyu-embed/src/bpe.rs 的 SPLIT 一字不差。
SPLIT = (
    r"[^\r\n\p{L}\p{N}]?[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]*[\p{Ll}\p{Lm}\p{Lo}\p{M}]+(?i:'s|'t|'re|'ve|'m|'ll|'d)?"
    r"|[^\r\n\p{L}\p{N}]?[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]+[\p{Ll}\p{Lm}\p{Lo}\p{M}]*(?i:'s|'t|'re|'ve|'m|'ll|'d)?"
    r"|\p{N}{1,3}| ?[^\s\p{L}\p{N}]+[\r\n/]*|\s*[\r\n]+|\s+(?!\S)|\s+"
)


def refuse(why):
    sys.exit("unsupported tokenizer: " + why)


def expect(what, got, want):
    if got != want:
        refuse(f"{what} is {json.dumps(got, ensure_ascii=False)}, want {json.dumps(want, ensure_ascii=False)}")


def main(source, out):
    tok = json.load(open(source, encoding="utf-8"))
    expect("normalizer", tok.get("normalizer"), None)
    pre = tok.get("pre_tokenizer") or {}
    expect("pre_tokenizer.type", pre.get("type"), "Sequence")
    steps = pre.get("pretokenizers") or []
    expect("number of pre_tokenizers", len(steps), 2)
    split, level = steps
    expect("split", {k: split.get(k) for k in ("type", "pattern", "behavior", "invert")},
           {"type": "Split", "pattern": {"Regex": SPLIT}, "behavior": "Isolated", "invert": False})
    expect("byte level", {k: level.get(k) for k in ("type", "add_prefix_space", "use_regex")},
           {"type": "ByteLevel", "add_prefix_space": False, "use_regex": False})
    model = tok["model"]
    expect("model.type", model.get("type"), "BPE")
    for key, want in (("dropout", None), ("unk_token", None), ("continuing_subword_prefix", None),
                      ("end_of_word_suffix", None), ("fuse_unk", False), ("byte_fallback", False),
                      ("ignore_merges", True)):
        expect("model." + key, model.get(key), want)
    post = tok.get("post_processor") or {}
    expect("post_processor.type", post.get("type"), "TemplateProcessing")
    single = post.get("single") or []
    if len(single) != 3 or "SpecialToken" not in single[0] or single[1] != {"Sequence": {"id": "A", "type_id": 0}} \
            or "SpecialToken" not in single[2]:
        refuse("post_processor.single is not <start> A <end>")
    start, end = single[0]["SpecialToken"]["id"], single[2]["SpecialToken"]["id"]

    vocab = model["vocab"]
    words = [None] * len(vocab)
    for word, i in vocab.items():
        if not 0 <= i < len(words) or words[i] is not None:
            refuse(f"vocab id {i} is out of order")
        words[i] = word
    for added in tok.get("added_tokens") or []:
        i, word = added["id"], added["content"]
        if i < len(words):
            if words[i] != word:
                refuse(f"added token {word} has the id of {words[i]}")
            continue
        if i != len(words):
            refuse(f"added token {word} leaves a gap at id {len(words)}")
        words.append(word)
    seen = set()
    for word in words:
        if word == "" or "\n" in word or "\r" in word:
            refuse(f"token {word!r} cannot be one line")
        if word in seen:
            refuse(f"token {word} is there twice")
        seen.add(word)
    for word in (start, end):
        if word not in seen:
            refuse(f"special token {word} is not in vocab")

    pairs = []
    for merge in model["merges"]:
        left, right = merge.split(" ", 1) if isinstance(merge, str) else merge
        if " " in left or " " in right:
            refuse(f"merge {left!r} {right!r} has a space")
        for word in (left, right, left + right):
            if word not in seen:
                refuse(f"merge {left} {right}: {word} is not in vocab")
        pairs.append(f"{left} {right}")

    os.makedirs(out, exist_ok=True)
    files = []
    for role, name, lines in (("vocab", "vocab.txt", words), ("merges", "merges.txt", pairs)):
        data = ("\n".join(lines) + "\n").encode("utf-8")
        open(os.path.join(out, name), "wb").write(data)
        files.append((role, name, hashlib.sha256(data).hexdigest(), len(data)))
    print("[tokenizer]")
    print('kind = "bpe"')
    print(f"start = {json.dumps(start, ensure_ascii=False)}")
    print(f"end = {json.dumps(end, ensure_ascii=False)}")
    for role, name, digest, size in files:
        print(f'\n[[files]]\nrole = "{role}"\nname = "{name}"\nsha256 = "{digest}"\nsize = {size}')


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit("usage: python -I bpe_files.py <tokenizer.json> <output dir>")
    main(sys.argv[1], sys.argv[2])
