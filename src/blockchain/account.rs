//! 账户管理模块
//! 负责地址生成、余额查询、nonce 管理等账户相关操作
//! 
//! 跨 crate 接口只使用 Rust 标准类型（String、字节数组、基础整数类型）

use super::rpc::RpcClient;
use super::utils;
use super::conversion;

/// 从私钥（十六进制字符串）提取公钥坐标
/// 
/// # 参数
/// - `private_key_hex`: 私钥的十六进制字符串（可带或不带 0x 前缀）
/// 
/// # 返回
/// - (x_hex, y_hex) 公钥的 x 和 y 坐标，均为 64 字符十六进制字符串
pub fn private_key_hex_to_public_key_xy(private_key_hex: &str) -> Result<(String, String), String>
{
    use ark_ec::{CurveGroup, PrimeGroup};
    use ark_secp256k1::Projective;

    let private_key = conversion::hex_to_fr(private_key_hex)?;
    let public_key = Projective::generator() * &private_key;
    let affine = public_key.into_affine();

    let x_bytes = conversion::fq_to_bytes32(&affine.x);
    let y_bytes = conversion::fq_to_bytes32(&affine.y);

    let x_hex = utils::bytes_to_hex(&x_bytes);
    let y_hex = utils::bytes_to_hex(&y_bytes);

    Ok((x_hex, y_hex))
}

/// 从私钥（十六进制字符串）生成以太坊地址
/// 
/// # 参数
/// - `private_key_hex`: 私钥的十六进制字符串（可带或不带 0x 前缀）
/// 
/// # 返回
/// - 以太坊地址（带 0x 前缀，EIP-55 校验和格式）
/// 
/// # 示例
/// ```
/// let address = private_key_hex_to_address("0x1234...abcd")?;
/// println!("Address: {}", address);
/// ```
pub fn private_key_hex_to_address(private_key_hex: &str) -> Result<String, String>
{
    let private_key = conversion::hex_to_fr(private_key_hex)?;
    Ok(conversion::fr_to_address(&private_key))
}

/// 查询账户余额
/// 
/// # 参数
/// - `rpc`: RPC 客户端
/// - `address`: 账户地址
/// 
/// # 返回
/// - 余额的十六进制字符串（Wei 单位，带 0x 前缀）
/// 
/// # 示例
/// ```
/// let balance_hex = get_balance(&rpc, "0x...").await?;
/// let ether = conversion::wei_hex_to_ether(&balance_hex)?;
/// ```
pub async fn get_balance(rpc: &RpcClient, address: &str) -> Result<String, String>
{
    let formatted = utils::format_address(address);
    rpc.eth_get_balance(&formatted, "latest").await
}
