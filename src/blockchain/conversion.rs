//! 类型转换模块
//! 提供 k256 和 ark-secp256k1 之间的类型转换
//! 
//! 核心原则：
//! 1. 跨 crate 接口只暴露 Rust 标准类型（String、字节数组、基础整数类型）
//! 2. 转换流程：String <---(长度检查)---> 字节数组 <-> k256或ark类型
//! 3. String 不直接与 k256/ark 类型接触

use ark_ff::{BigInteger, PrimeField};
use ark_secp256k1::{Fr, Fq, Projective};
use ark_ec::CurveGroup;
use k256::elliptic_curve::bigint::U256;
use super::utils;

// ============ 字节数组 <-> k256 类型 ============

/// 将 32 字节数组转换为 k256 的 U256
/// 
/// # 参数
/// - `bytes`: 32 字节数组（大端序）
/// 
/// # 返回
/// - k256::U256 值
pub(crate) fn bytes32_to_k256_u256(bytes: &[u8; 32]) -> U256
{
    U256::from_be_slice(bytes)
}

// ============ 字节数组 <-> ark 类型 ============

/// 将 32 字节数组转换为 ark Fr（标量场元素）
/// 
/// # 参数
/// - `bytes`: 32 字节数组（大端序）
/// 
/// # 返回
/// - ark_secp256k1::Fr 值
pub(crate) fn bytes32_to_fr(bytes: &[u8; 32]) -> Fr
{
    Fr::from_be_bytes_mod_order(bytes)
}

/// 将 ark Fq 转换为 32 字节数组
/// 
/// # 参数
/// - `fq`: ark_secp256k1::Fq 值
/// 
/// # 返回
/// - 32 字节数组（大端序）
pub(crate) fn fq_to_bytes32(fq: &Fq) -> [u8; 32]
{
    let bigint = fq.into_bigint();
    let bytes = bigint.to_bytes_be();
    // 确保是 32 字节
    let mut result = [0u8; 32];
    let start = 32 - bytes.len().min(32);
    result[start..].copy_from_slice(&bytes[..bytes.len().min(32)]);
    result
}

// ============ 组合转换：String <-> k256 类型（经过字节数组）============

/// 将十六进制字符串转换为 k256 的 U256
/// 
/// 转换流程：String -> 字节数组（长度检查）-> k256::U256
/// 
/// # 参数
/// - `hex`: 十六进制字符串（可带或不带 0x 前缀）
/// 
/// # 返回
/// - k256::U256 值
/// 
/// # 错误
/// - 如果字符串格式无效
/// 
/// 注意：此函数仅在 crate 内部使用
pub(crate) fn hex_to_k256_u256(hex: &str) -> Result<U256, String>
{
    let bytes = utils::hex_to_bytes32(hex)?;
    Ok(bytes32_to_k256_u256(&bytes))
}

// ============ 组合转换：String <-> ark 类型（经过字节数组）============

/// 将十六进制字符串转换为 ark Fr
/// 
/// 转换流程：String -> 字节数组（长度检查）-> ark::Fr
/// 
/// # 参数
/// - `hex`: 十六进制字符串（可带或不带 0x 前缀）
/// 
/// # 返回
/// - ark_secp256k1::Fr 值
/// 
/// # 错误
/// - 如果字符串格式无效
/// 
/// 注意：此函数仅在 crate 内部使用
pub(crate) fn hex_to_fr(hex: &str) -> Result<Fr, String>
{
    let bytes = utils::hex_to_bytes32(hex)?;
    Ok(bytes32_to_fr(&bytes))
}

// ============ 地址转换 ============

/// 将 ark Projective 点转换为以太坊地址
/// 
/// # 参数
/// - `point`: ark_secp256k1::Projective 公钥点
/// 
/// # 返回
/// - 以太坊地址（带 0x 前缀，EIP-55 校验和格式）
/// 
/// 注意：此函数仅在 crate 内部使用
pub(crate) fn projective_to_address(point: &Projective) -> String
{
    let affine = point.into_affine();
    
    // 通过字节数组获取坐标
    let x_bytes = fq_to_bytes32(&affine.x);
    let y_bytes = fq_to_bytes32(&affine.y);
    
    // 构建 64 字节的公钥（不含 0x04 前缀）
    let mut public_key_bytes = [0u8; 64];
    public_key_bytes[0..32].copy_from_slice(&x_bytes);
    public_key_bytes[32..64].copy_from_slice(&y_bytes);
    
    // Keccak256 哈希后取最后 20 字节
    let hash = utils::keccak256(&public_key_bytes);
    let address_bytes = &hash[12..32];
    
    // 字节数组 -> String
    let address = utils::bytes_to_hex(address_bytes);
    utils::to_checksum_address(&address)
}

/// 将 ark Fr（私钥）转换为以太坊地址
/// 
/// # 参数
/// - `private_key`: ark_secp256k1::Fr 私钥
/// 
/// # 返回
/// - 以太坊地址（带 0x 前缀，EIP-55 校验和格式）
/// 
/// 注意：此函数仅在 crate 内部使用
pub(crate) fn fr_to_address(private_key: &Fr) -> String
{
    use ark_ec::PrimeGroup;
    let public_key = Projective::generator() * private_key;
    projective_to_address(&public_key)
}

// ============ 公开的验证和解析函数 ============

/// 验证私钥字符串格式是否有效
/// 
/// 转换流程：String -> 字节数组（长度检查）-> ark::Fr -> 验证非零
/// 
/// # 参数
/// - `hex`: 十六进制私钥字符串
/// 
/// # 返回
/// - 是否有效
pub fn is_valid_private_key(hex: &str) -> bool
{
    let hex_str = utils::remove_0x_prefix(hex);
    
    // 基础格式检查
    if hex_str.is_empty() || hex_str.len() > 64
    {
        return false;
    }
    if !hex_str.chars().all(|c| c.is_ascii_hexdigit())
    {
        return false;
    }
    
    // String -> 字节数组（带长度检查）
    match utils::hex_to_bytes32(hex)
    {
        Ok(bytes) =>
        {
            // 字节数组 -> ark::Fr
            let fr = bytes32_to_fr(&bytes);
            // 验证非零
            fr != Fr::from(0u64)
        }
        Err(_) => false,
    }
}

