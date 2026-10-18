"""把 granite-embedding-97m-multilingual-r2 的 fp32 ONNX 量化成 4 位、整数计算的那一份（施工 R-5 七补，`docs/blueprint/recall.md`
第四条第 2 款）：

    python -I quantize_q4.py <onnx/model.onnx> <输出的 model_q4.onnx>

要 onnxruntime 1.31、onnx。MatMul 和 Gather（词嵌入）都量成 4 位，一块 32 个、对称；MatMul 照第 0 轴、Gather 照第 1 轴；
accuracy_level=4：激活量成 8 位整数算，比照 fp32 算快（Gather 的 4 位要 ONNX Runtime 1.20 以后认的 GatherBlockQuantized）。
2026-10-18 量：61 MB，和 fp32 的向量平均余弦 0.99，召回和官方的 8 位一样（`memory/eval.md`）。
"""
import sys
import onnx
from onnxruntime.quantization.matmul_nbits_quantizer import DefaultWeightOnlyQuantConfig, MatMulNBitsQuantizer

source, out = sys.argv[1], sys.argv[2]
config = DefaultWeightOnlyQuantConfig(
    block_size=32, is_symmetric=True, accuracy_level=4,
    op_types_to_quantize=("MatMul", "Gather"), quant_axes=(("MatMul", 0), ("Gather", 1)))
quantizer = MatMulNBitsQuantizer(onnx.load(source), algo_config=config)
quantizer.process()
quantizer.model.save_model_to_file(out, use_external_data_format=False)
