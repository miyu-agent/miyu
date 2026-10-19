//! 手造的小模型（`tests/fixtures/embed-tiny/`，`make.py` 照种子 7 造的）的手算：每个位置是词的嵌入加第 0 类的嵌入，第一格
//! 再加上整句的和，取第一格、归一化。和 ONNX Runtime 算的差不过 1e-6（`tests/embed.rs` 照 `expected.json` 比）。

use sha2::{Digest, Sha256};

/// 维数。
pub const DIMS: usize = 4;

/// 小模型文件的 SHA-256。
const SHA256: &str = "b3b1a8dc7601a46d0aa8b9065d7a7b6b892ece18fd642734013303ab76f85a89";

/// 每个词的嵌入，照词表的行号。
const WORDS: [[f32; DIMS]; 21] = [
    [0.001, 0.299, -0.274, -0.891],
    [-0.455, -0.992, 0.060, 1.340],
    [-0.492, -0.620, 0.490, 0.357],
    [0.105, -0.930, -0.029, 0.695],
    [-1.344, -0.458, -1.901, -1.290],
    [-1.842, -0.235, -1.267, 0.271],
    [0.157, -0.187, -2.517, -0.539],
    [-0.049, 0.113, -1.530, -0.478],
    [-0.979, -0.809, 1.061, -0.808],
    [-0.033, 0.884, -0.584, -0.112],
    [0.110, 0.064, -1.225, 0.076],
    [1.359, -1.547, 0.859, 0.119],
    [-0.641, 2.000, 0.762, -1.199],
    [0.075, 0.577, -0.189, 0.683],
    [-0.067, 0.667, 1.439, -0.676],
    [0.203, -0.463, 0.127, -1.187],
    [-0.579, -0.196, 0.899, 1.145],
    [-1.324, -0.795, 0.647, -1.992],
    [-0.463, -0.097, 1.257, 0.689],
    [-0.327, -0.369, -0.250, 1.524],
    [-0.428, -0.304, 0.353, -0.121],
];

/// 第 0 类的嵌入（`token_type_ids` 全是 0）。
const TYPE: [f32; DIMS] = [-0.197, -1.114, -0.012, -0.444];

/// `model` 是不是小模型的原样。
pub fn is_tiny(model: &[u8]) -> bool {
    let digest: String = Sha256::digest(model)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    digest == SHA256
}

/// 照编号算：词表外的编号算不出。全是零的没有。
pub fn embed(ids: &[i64]) -> Option<Vec<f32>> {
    let mut rows = Vec::with_capacity(ids.len());
    for id in ids {
        let word = WORDS.get(usize::try_from(*id).ok()?)?;
        rows.push(std::array::from_fn::<f32, DIMS, _>(|at| {
            word[at] + TYPE[at]
        }));
    }
    let first = rows.first()?;
    let vector: Vec<f32> = (0..DIMS)
        .map(|at| first[at] + rows.iter().map(|row| row[at]).sum::<f32>())
        .collect();
    let length = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    (length > 0.0 && length.is_finite()).then(|| vector.iter().map(|x| x / length).collect())
}
