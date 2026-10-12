知识库的 PDF 测试数据（施工 K-5，`docs/blueprint/knowledge.md`「怎么走」第二条）：

- `cat.pdf`：三页，第一页中文、第二页英文、第三页中文带数字。照 `cat-source.html` 用 Chromium 打出来的：

  ```sh
  chromium --headless --disable-gpu --no-pdf-header-footer --print-to-pdf=cat.pdf cat-source.html
  ```

  Chrome 打的 PDF 把一些字映射成部首的码位（「子」是 U+2F26、「页」是 U+2EDA），测试照这个查换没换回来。
- `blank.pdf`：一页，只有一张画，一个字都抽不出来（同扫描件）。照 `blank-source.html` 同样打出来。
