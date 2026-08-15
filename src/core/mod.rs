//! 核心逻辑：定位编辑器脚本包、XOR 加解密、备份、日志、内核补丁、补丁模块管理。

pub mod backup;
pub mod crypto;
pub mod kernel;
pub mod locate;
pub mod log;
pub mod modules;
pub mod ops;

pub use locate::{locate, EditorTarget};
