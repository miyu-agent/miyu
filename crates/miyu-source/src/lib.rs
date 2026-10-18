//! 软件源（设计 `37-软件源.md`）：包文件怎么打、怎么解（施工 G-1，[`archive`]），索引的写法和签名（施工 G-2 上，[`index`]、
//! [`sign`]）。之后的收录、下载也住这里。

pub mod archive;
pub mod index;
pub mod sign;

#[cfg(test)]
mod test_support;
