/// RPC 客户端模块
/// 负责与 EVM 链节点通信，封装所有 JSON-RPC 调用
pub mod rpc;

/// 交易构建模块
/// 负责构建、签名和序列化交易（包括 Legacy、EIP-1559 等类型）
pub mod transaction;

/// 账户管理模块
/// 负责地址生成、余额查询、nonce 管理等账户相关操作
pub mod account;

/// 合约交互模块
/// 负责合约调用、合约部署、ABI 编解码
pub mod contract;

/// 工具函数模块
/// 提供通用的工具函数（单位转换、地址校验、十六进制处理等）
pub mod utils;

/// Blockscout v2 API 模块
/// 提供通过 Blockscout API 获取交易历史、代币转移记录等功能
pub mod blockscout;

/// 钱包只读请求模块
/// 提供余额和交易历史等纯网络请求，不依赖 UI 状态
pub mod read;

/// 转账请求模块
/// 提供原生代币和 ERC20 发送请求，不依赖 UI 状态
pub mod transfer;

/// 类型转换模块
/// 提供 k256 和 ark-secp256k1 之间的类型转换，确保跨 crate 接口不暴露 k256 类型
pub mod conversion;

/// 交换模块
/// 使用 Uniswap V3 实现代币交换功能
pub mod swap;
