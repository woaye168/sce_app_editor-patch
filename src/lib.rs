//! 库入口：把 core 暴露为库目标，供 examples（make_slots/decrypt_mirror）复用内核实现；
//! cli/mcp 为 0.5.4 新增的应用自持编辑器控制能力（与 bgd_sce_tools 解耦）。

pub mod cli;
pub mod core;
pub mod mcp;
pub mod scenario;
