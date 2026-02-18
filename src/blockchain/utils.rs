//! 工具函数模块
//! 提供通用的工具函数（单位转换、地址校验、十六进制处理等）

use sha3::{Digest, Keccak256};

/// 将 Wei 转换为 Ether
/// 
/// # 参数
/// - `wei`: Wei 数量（u128）
/// 
/// # 返回
/// - Ether 数量（f64）
/// 
/// # 示例
/// ```
/// let ether = wei_to_ether(1_000_000_000_000_000_000u128);
/// assert_eq!(ether, 1.0);
/// ```
pub fn wei_to_ether(wei: u128) -> f64
{
    let divisor = 1_000_000_000_000_000_000u128;
    let whole = wei / divisor;
    let remainder = wei % divisor;
    
    let whole_f64 = whole as f64;
    let remainder_f64 = remainder as f64 / 1e18;
    
    whole_f64 + remainder_f64
}

/// 将 Ether 转换为 Wei
/// 
/// # 参数
/// - `ether`: Ether 数量（f64）
/// 
/// # 返回
/// - Wei 数量（u128）
/// 
/// # 示例
/// ```
/// let wei = ether_to_wei(1.5);
/// ```
pub fn ether_to_wei(ether: f64) -> u128
{
    let wei = ether * 1e18;
    wei as u128
}

/// 将 Gwei 转换为 Wei
/// 
/// # 参数
/// - `gwei`: Gwei 数量（u64）
/// 
/// # 返回
/// - Wei 数量（u128）
pub fn gwei_to_wei(gwei: u64) -> u128
{
    (gwei as u128) * 1_000_000_000u128
}

/// 将 Wei 转换为 Gwei
/// 
/// # 参数
/// - `wei`: Wei 数量（u128）
/// 
/// # 返回
/// - Gwei 数量（u64）
pub fn wei_to_gwei(wei: u128) -> u64
{
    (wei / 1_000_000_000u128) as u64
}

/// 将十六进制字符串转换为字节数组
/// 
/// # 参数
/// - `hex`: 十六进制字符串（可带或不带 0x 前缀）
/// 
/// # 返回
/// - 字节数组
/// 
/// # 示例
/// ```
/// let bytes = hex_to_bytes("0x1234").unwrap();
/// assert_eq!(bytes, vec![0x12, 0x34]);
/// ```
pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String>
{
    let hex = remove_0x_prefix(hex);
    if hex.len() % 2 != 0
    {
        return Err("Hex string must have even length".into());
    }
    
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| format!("Invalid hex character at position {}", i))
        })
        .collect()
}

/// 将字节数组转换为十六进制字符串（不带 0x 前缀）
/// 
/// # 参数
/// - `bytes`: 字节数组
/// 
/// # 返回
/// - 十六进制字符串
pub fn bytes_to_hex(bytes: &[u8]) -> String
{
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// 添加 0x 前缀
/// 
/// # 参数
/// - `hex`: 十六进制字符串
/// 
/// # 返回
/// - 带 0x 前缀的字符串
pub fn add_0x_prefix(hex: &str) -> String
{
    if hex.starts_with("0x") || hex.starts_with("0X")
    {
        hex.to_lowercase()
    }
    else
    {
        format!("0x{}", hex.to_lowercase())
    }
}

/// 移除 0x 前缀
/// 
/// # 参数
/// - `hex`: 十六进制字符串
/// 
/// # 返回
/// - 不带 0x 前缀的字符串
pub fn remove_0x_prefix(hex: &str) -> &str
{
    if hex.starts_with("0x") || hex.starts_with("0X")
    {
        &hex[2..]
    }
    else
    {
        hex
    }
}

/// 计算 Keccak256 哈希
/// 
/// # 参数
/// - `data`: 要哈希的数据
/// 
/// # 返回
/// - 32 字节的哈希值
pub fn keccak256(data: &[u8]) -> [u8; 32]
{
    let mut hasher = Keccak256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// 计算函数选择器（函数签名的 Keccak256 哈希的前 4 字节）
/// 
/// # 参数
/// - `signature`: 函数签名，如 "transfer(address,uint256)"
/// 
/// # 返回
/// - 4 字节的函数选择器
/// 
/// # 示例
/// ```
/// let selector = function_selector("transfer(address,uint256)");
/// ```
pub fn function_selector(signature: &str) -> [u8; 4]
{
    let hash = keccak256(signature.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}

/// 格式化地址（统一为小写，带 0x 前缀，40 个字符）
/// 
/// # 参数
/// - `address`: 以太坊地址
/// 
/// # 返回
/// - 格式化后的地址
pub fn format_address(address: &str) -> String
{
    let addr = remove_0x_prefix(address).to_lowercase();
    let padded = format!("{:0>40}", addr);
    format!("0x{}", padded)
}

/// 验证地址格式是否正确
/// 
/// # 参数
/// - `address`: 以太坊地址
/// 
/// # 返回
/// - 是否为有效地址
pub fn is_valid_address(address: &str) -> bool
{
    let addr = remove_0x_prefix(address);
    addr.len() == 40 && addr.chars().all(|c| c.is_ascii_hexdigit())
}

/// 将地址转换为 EIP-55 校验和格式
/// 
/// # 参数
/// - `address`: 以太坊地址
/// 
/// # 返回
/// - EIP-55 校验和格式的地址
pub fn to_checksum_address(address: &str) -> String
{
    let addr = remove_0x_prefix(address).to_lowercase();
    let hash = keccak256(addr.as_bytes());
    let hash_hex = bytes_to_hex(&hash);
    
    let checksum: String = addr
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_ascii_digit()
            {
                c
            }
            else
            {
                let hash_char = hash_hex.chars().nth(i).unwrap();
                let hash_val = hash_char.to_digit(16).unwrap();
                if hash_val >= 8 { c.to_ascii_uppercase() } else { c }
            }
        })
        .collect();
      format!("0x{}", checksum)
}

/// 将 u128 转换为十六进制字符串（带 0x 前缀）
/// 
/// # 参数
/// - `value`: u128 值
/// 
/// # 返回
/// - 十六进制字符串
pub fn u128_to_hex(value: u128) -> String
{
    if value == 0
    {
        "0x0".to_string()
    }
    else
    {
        format!("0x{:x}", value)
    }
}

/// 将十六进制字符串转换为 u128（带溢出检查）
/// 
/// # 参数
/// - `hex`: 十六进制字符串（可带 0x 前缀）
/// 
/// # 返回
/// - u128 值
/// 
/// # 错误
/// - 如果字符串格式无效
/// - 如果值超过 u128::MAX（32 个十六进制字符，16 字节）
pub fn hex_to_u128(hex: &str) -> Result<u128, String>
{
    let hex = remove_0x_prefix(hex);
    
    // 检查长度：u128 最多 32 个十六进制字符（16 字节）
    if hex.len() > 32
    {
        return Err(format!(
            "Hex string too long for u128 (max 32 chars, got {}): 0x{}",
            hex.len(), 
            if hex.len() > 64 { &hex[..64] } else { hex }
        ));
    }
    
    u128::from_str_radix(hex, 16).map_err(|e| format!("Failed to parse hex as u128: {}", e))
}

/// 将 u64 转换为十六进制字符串（带 0x 前缀）
/// 
/// # 参数
/// - `value`: u64 值
/// 
/// # 返回
/// - 十六进制字符串
pub fn u64_to_hex(value: u64) -> String
{
    format!("0x{:x}", value)
}

/// 将十六进制字符串转换为 u64
/// 
/// # 参数
/// - `hex`: 十六进制字符串（可带 0x 前缀）
/// 
/// # 返回
/// - u64 值
pub fn hex_to_u64(hex: &str) -> Result<u64, String>
{
    let hex = remove_0x_prefix(hex);
    u64::from_str_radix(hex, 16).map_err(|e| format!("Failed to parse hex: {}", e))
}

/// 将 Ether 转换为 Wei（返回十六进制字符串）
/// 
/// # 参数
/// - `ether`: Ether 数量（f64）
/// 
/// # 返回
/// - Wei 数量的十六进制字符串
pub fn ether_to_wei_hex(ether: f64) -> String
{
    let wei = (ether * 1e18) as u128;
    format!("0x{:x}", wei)
}

/// 将 Gwei 转换为 Wei（返回十六进制字符串）
/// 
/// # 参数
/// - `gwei`: Gwei 数量
/// 
/// # 返回
/// - Wei 数量的十六进制字符串
pub fn gwei_to_wei_hex(gwei: u64) -> String
{
    let wei = gwei as u128 * 1_000_000_000u128;
    format!("0x{:x}", wei)
}

/// 将 Wei 十六进制字符串转换为 Ether
/// 
/// # 参数
/// - `wei_hex`: Wei 数量的十六进制字符串
/// 
/// # 返回
/// - Ether 数量（f64）
pub fn wei_hex_to_ether(wei_hex: &str) -> Result<f64, String>
{
    let wei = hex_to_u128(wei_hex)?;
    Ok(wei_to_ether(wei))
}

/// 将 Wei 十六进制字符串转换为 u128
/// 
/// # 参数
/// - `wei_hex`: Wei 的十六进制字符串
/// 
/// # 返回
/// - u128 Wei 值
pub fn wei_hex_to_u128(wei_hex: &str) -> Result<u128, String>
{
    hex_to_u128(wei_hex)
}

/// 将 u128 Wei 转换为十六进制字符串
/// 
/// # 参数
/// - `wei`: Wei 数量（u128）
/// 
/// # 返回
/// - 十六进制字符串（带 0x 前缀）
pub fn wei_u128_to_hex(wei: u128) -> String
{
    u128_to_hex(wei)
}

/// 将十六进制字符串转换为 32 字节数组（用于 ABI 编码）
/// 
/// # 参数
/// - `hex`: 十六进制字符串（可带或不带 0x 前缀）
/// 
/// # 返回
/// - 32 字节数组（大端序）
/// 
/// # 错误
/// - 如果字符串格式无效或超过 64 个十六进制字符（32 字节）
pub fn hex_to_bytes32(hex: &str) -> Result<[u8; 32], String>
{
    let hex = remove_0x_prefix(hex);
    
    // 长度检查：最多 64 个十六进制字符（32 字节）
    if hex.len() > 64
    {
        return Err(format!("Hex string too long: {} chars (max 64)", hex.len()));
    }
    
    let padded = format!("{:0>64}", hex);
    let bytes = hex_to_bytes(&padded)?;
    if bytes.len() != 32
    {
        return Err("Invalid hex length for bytes32".into());
    }
    let mut result = [0u8; 32];
    result.copy_from_slice(&bytes);
    Ok(result)
}

/// 将 32 字节数组转换为十六进制字符串（带 0x 前缀）
/// 
/// # 参数
/// - `bytes`: 32 字节数组
/// 
/// # 返回
/// - 十六进制字符串
pub fn bytes32_to_hex(bytes: &[u8; 32]) -> String
{
    let hex = bytes_to_hex(bytes);
    let trimmed = hex.trim_start_matches('0');
    if trimmed.is_empty()
    {
        "0x0".to_string()
    }
    else
    {
        format!("0x{}", trimmed)
    }
}

/// 将 u64 转换为 32 字节数组（大端序，用于 ABI 编码）
/// 
/// # 参数
/// - `value`: u64 值
/// 
/// # 返回
/// - 32 字节数组（前 24 字节为 0，后 8 字节为值）
pub fn u64_to_bytes32(value: u64) -> [u8; 32]
{
    let mut result = [0u8; 32];
    result[24..32].copy_from_slice(&value.to_be_bytes());
    result
}

/// 将 32 字节数组转换为 u64（大端序，用于 ABI 解码）
/// 
/// # 参数
/// - `bytes`: 32 字节数组
/// 
/// # 返回
/// - u64 值（仅使用最后 8 字节）
/// 
/// # 错误
/// - 如果值超出 u64 范围（前 24 字节不全为 0）
pub fn bytes32_to_u64(bytes: &[u8; 32]) -> Result<u64, String>
{
    // 检查前 24 字节是否全为 0
    if bytes[0..24].iter().any(|&b| b != 0)
    {
        return Err("Value too large for u64".into());
    }
    
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&bytes[24..32]);
    Ok(u64::from_be_bytes(arr))
}

/// 将 usize 转换为 32 字节数组（大端序，用于 ABI 编码）
/// 
/// # 参数
/// - `value`: usize 值
/// 
/// # 返回
/// - 32 字节数组
pub fn usize_to_bytes32(value: usize) -> [u8; 32]
{
    // 在 64 位系统上，usize 等同于 u64
    u64_to_bytes32(value as u64)
}

/// 将 bool 转换为 32 字节数组（用于 ABI 编码）
/// 
/// # 参数
/// - `value`: bool 值
/// 
/// # 返回
/// - 32 字节数组（false = 全 0，true = 最后一字节为 1）
pub fn bool_to_bytes32(value: bool) -> [u8; 32]
{
    let mut result = [0u8; 32];
    if value
    {
        result[31] = 1;
    }
    result
}
