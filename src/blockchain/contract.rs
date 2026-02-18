//! 合约交互模块
//! 负责合约调用、合约部署、ABI 编解码

use super::rpc::RpcClient;
use super::transaction::{Transaction, TransactionBuilder, TxType};
use super::utils;
use serde_json::json;

/// ABI 参数类型
#[derive(Debug, Clone)]
pub enum ParamType
{
    /// 地址类型（20 字节）
    Address,
    /// 无符号整数（位数，如 256）
    Uint(usize),
    /// 有符号整数
    Int(usize),
    /// 布尔类型
    Bool,
    /// 固定长度字节数组
    FixedBytes(usize),
    /// 动态字节数组
    Bytes,
    /// 字符串
    String,
    /// 动态数组
    Array(Box<ParamType>),
}

/// ABI 编码后的值
#[derive(Debug, Clone)]
pub enum Token
{
    /// 地址
    Address(String),
    /// 无符号整数（十六进制字符串，带 0x 前缀）
    Uint(String),
    /// 有符号整数（十六进制字符串，带 0x 前缀）
    Int(String),
    /// 布尔
    Bool(bool),
    /// 固定字节
    FixedBytes(Vec<u8>),
    /// 动态字节
    Bytes(Vec<u8>),
    /// 字符串
    String(String),
    /// 数组
    Array(Vec<Token>),
}

/// 编码函数调用数据
/// 
/// # 参数
/// - `function_signature`: 函数签名，如 "transfer(address,uint256)"
/// - `params`: 参数列表
/// 
/// # 返回
/// - 编码后的调用数据（包含函数选择器）
/// 
/// # 示例
/// ```
/// let data = encode_function_call(
///     "transfer(address,uint256)",
///     vec![
///         Token::Address("0x...".into()),
///         Token::Uint(U256::from(1000000u64)),
///     ]
/// )?;
/// ```
pub fn encode_function_call(function_signature: &str, params: Vec<Token>) -> Result<Vec<u8>, String>
{
    let selector = utils::function_selector(function_signature);
    let encoded_params = encode_params(&params)?;
    
    let mut result = selector.to_vec();
    result.extend_from_slice(&encoded_params);
    Ok(result)
}

/// 编码参数列表
fn encode_params(params: &[Token]) -> Result<Vec<u8>, String>
{
    if params.is_empty()
    {
        return Ok(Vec::new());
    }

    let mut head = Vec::new();
    let mut tail = Vec::new();
    let mut head_offset = params.len() * 32;    for token in params
    {
        if is_dynamic(token)
        {
            // 动态类型：head 中存偏移量
            head.extend_from_slice(&encode_uint(utils::usize_to_bytes32(head_offset)));
            let encoded = encode_token(token)?;
            head_offset += encoded.len();
            tail.extend_from_slice(&encoded);
        }
        else
        {
            // 静态类型：直接编码到 head
            head.extend_from_slice(&encode_token(token)?);
        }
    }

    head.extend_from_slice(&tail);
    Ok(head)
}

/// 判断是否为动态类型
fn is_dynamic(token: &Token) -> bool
{
    matches!(token, Token::Bytes(_) | Token::String(_) | Token::Array(_))
}

/// 编码单个 Token
fn encode_token(token: &Token) -> Result<Vec<u8>, String>
{
    match token
    {
        Token::Address(addr) =>
        {
            let bytes = utils::hex_to_bytes(addr)?;
            Ok(pad_left(&bytes, 32))
        }
        Token::Uint(value_hex) =>
        {
            let value = utils::hex_to_bytes32(value_hex)?;
            Ok(encode_uint(value))
        }
        Token::Int(value_hex) =>
        {
            let value = utils::hex_to_bytes32(value_hex)?;
            Ok(encode_uint(value)) // 简化处理
        }
        Token::Bool(b) =>
        {
            let value = utils::bool_to_bytes32(*b);
            Ok(encode_uint(value))
        }
        Token::FixedBytes(bytes) => Ok(pad_right(bytes, 32)),
        Token::Bytes(bytes) =>
        {
            let mut result = encode_uint(utils::usize_to_bytes32(bytes.len()));
            result.extend_from_slice(&pad_right(bytes, ((bytes.len() + 31) / 32) * 32));
            Ok(result)
        }
        Token::String(s) =>
        {
            let bytes = s.as_bytes();
            let mut result = encode_uint(utils::usize_to_bytes32(bytes.len()));
            result.extend_from_slice(&pad_right(bytes, ((bytes.len() + 31) / 32) * 32));
            Ok(result)
        }
        Token::Array(tokens) =>
        {
            let mut result = encode_uint(utils::usize_to_bytes32(tokens.len()));
            result.extend_from_slice(&encode_params(tokens)?);
            Ok(result)
        }
    }
}

/// 编码 32 字节数组
fn encode_uint(value: [u8; 32]) -> Vec<u8>
{
    value.to_vec()
}

/// 左填充到指定长度
fn pad_left(data: &[u8], len: usize) -> Vec<u8>
{
    let mut result = vec![0u8; len.saturating_sub(data.len())];
    result.extend_from_slice(data);
    result
}

/// 右填充到指定长度
fn pad_right(data: &[u8], len: usize) -> Vec<u8>
{
    let mut result = data.to_vec();
    result.resize(len, 0);
    result
}

/// 将 32 字节数组转换为 u64（内部使用）
fn bytes32_to_u64_internal(bytes: &[u8; 32]) -> u64
{
    u64::from_be_bytes([
        bytes[24], bytes[25], bytes[26], bytes[27],
        bytes[28], bytes[29], bytes[30], bytes[31],
    ])
}

/// 解码函数返回值
/// 
/// # 参数
/// - `data`: 返回数据（十六进制字符串或字节数组）
/// - `types`: 返回值类型列表
/// 
/// # 返回
/// - Token 列表
/// 
/// # 示例
/// ```
/// let result = decode_output("0x...", vec![ParamType::Uint(256)])?;
/// if let Token::Uint(value) = &result[0] {
///     println!("Value: {}", value);
/// }
/// ```
pub fn decode_output(data: &str, types: Vec<ParamType>) -> Result<Vec<Token>, String>
{
    let bytes = utils::hex_to_bytes(data)?;
    decode_params(&bytes, &types)
}

/// 解码参数
fn decode_params(data: &[u8], types: &[ParamType]) -> Result<Vec<Token>, String>
{
    let mut result = Vec::new();
    let mut offset = 0;

    for param_type in types
    {
        let (token, new_offset) = decode_param(data, offset, param_type)?;
        result.push(token);
        offset = new_offset;
    }

    Ok(result)
}

/// 解码单个参数
fn decode_param(data: &[u8], offset: usize, param_type: &ParamType) -> Result<(Token, usize), String>
{
    match param_type
    {
        ParamType::Address =>
        {
            if offset + 32 > data.len()
            {
                return Err("Not enough data for address".into());
            }
            let addr_bytes = &data[offset + 12..offset + 32];
            let addr = utils::add_0x_prefix(&utils::bytes_to_hex(addr_bytes));
            Ok((Token::Address(addr), offset + 32))
        }        ParamType::Uint(_) =>
        {
            if offset + 32 > data.len()
            {
                return Err("Not enough data for uint".into());
            }
            let mut bytes32 = [0u8; 32];
            bytes32.copy_from_slice(&data[offset..offset + 32]);
            let value_hex = utils::bytes32_to_hex(&bytes32);
            Ok((Token::Uint(value_hex), offset + 32))
        }
        ParamType::Int(_) =>
        {
            if offset + 32 > data.len()
            {
                return Err("Not enough data for int".into());
            }
            let mut bytes32 = [0u8; 32];
            bytes32.copy_from_slice(&data[offset..offset + 32]);
            let value_hex = utils::bytes32_to_hex(&bytes32);
            Ok((Token::Int(value_hex), offset + 32))
        }
        ParamType::Bool =>
        {
            if offset + 32 > data.len()
            {
                return Err("Not enough data for bool".into());
            }
            let value = data[offset + 31] != 0;
            Ok((Token::Bool(value), offset + 32))
        }
        ParamType::FixedBytes(size) =>
        {
            if offset + 32 > data.len()
            {
                return Err("Not enough data for fixed bytes".into());
            }
            let bytes = data[offset..offset + size].to_vec();
            Ok((Token::FixedBytes(bytes), offset + 32))
        }        ParamType::Bytes =>
        {
            // 动态类型：先读偏移量
            let mut offset_bytes = [0u8; 32];
            offset_bytes.copy_from_slice(&data[offset..offset + 32]);
            let data_offset = bytes32_to_u64_internal(&offset_bytes) as usize;
            
            let mut length_bytes = [0u8; 32];
            length_bytes.copy_from_slice(&data[data_offset..data_offset + 32]);
            let length = bytes32_to_u64_internal(&length_bytes) as usize;
            
            let bytes = data[data_offset + 32..data_offset + 32 + length].to_vec();
            Ok((Token::Bytes(bytes), offset + 32))
        }
        ParamType::String =>
        {
            let mut offset_bytes = [0u8; 32];
            offset_bytes.copy_from_slice(&data[offset..offset + 32]);
            let data_offset = bytes32_to_u64_internal(&offset_bytes) as usize;
            
            let mut length_bytes = [0u8; 32];
            length_bytes.copy_from_slice(&data[data_offset..data_offset + 32]);
            let length = bytes32_to_u64_internal(&length_bytes) as usize;
            
            let bytes = &data[data_offset + 32..data_offset + 32 + length];
            let s = String::from_utf8(bytes.to_vec())
                .map_err(|_| "Invalid UTF-8 string")?;
            Ok((Token::String(s), offset + 32))
        }
        ParamType::Array(_) =>
        {
            // 简化：暂不支持数组解码
            Err("Array decoding not implemented".into())
        }
    }
}

/// 调用合约（只读，不发送交易）
/// 
/// # 参数
/// - `rpc`: RPC 客户端
/// - `contract_address`: 合约地址
/// - `data`: 调用数据
/// 
/// # 返回
/// - 返回数据（十六进制字符串）
/// 
/// # 示例
/// ```
/// let data = encode_function_call("balanceOf(address)", vec![Token::Address(addr)])?;
/// let result = call_contract(&rpc, "0x...", data).await?;
/// ```
pub async fn call_contract(
    rpc: &RpcClient,
    contract_address: &str,
    data: Vec<u8>,
) -> Result<String, String>
{
    let tx = json!({
        "to": utils::format_address(contract_address),
        "data": utils::add_0x_prefix(&utils::bytes_to_hex(&data)),
    });
    
    rpc.eth_call(tx, "latest").await
}

/// 调用合约（指定调用者）
/// 
/// # 参数
/// - `rpc`: RPC 客户端
/// - `from`: 调用者地址
/// - `contract_address`: 合约地址
/// - `data`: 调用数据
/// 
/// # 返回
/// - 返回数据
pub async fn call_contract_from(
    rpc: &RpcClient,
    from: &str,
    contract_address: &str,
    data: Vec<u8>,
) -> Result<String, String>
{
    let tx = json!({
        "from": utils::format_address(from),
        "to": utils::format_address(contract_address),
        "data": utils::add_0x_prefix(&utils::bytes_to_hex(&data)),
    });
    
    rpc.eth_call(tx, "latest").await
}

/// 估算合约调用的 Gas
/// 
/// # 参数
/// - `rpc`: RPC 客户端
/// - `from`: 调用者地址
/// - `contract_address`: 合约地址
/// - `data`: 调用数据
/// - `value_hex`: 附带的 ETH（Wei，十六进制字符串）
/// 
/// # 返回
/// - 估算的 Gas 数量
pub async fn estimate_gas(
    rpc: &RpcClient,
    from: &str,
    contract_address: &str,
    data: Vec<u8>,
    value_hex: &str,
) -> Result<u64, String>
{
    let tx = json!({
        "from": utils::format_address(from),
        "to": utils::format_address(contract_address),
        "data": utils::add_0x_prefix(&utils::bytes_to_hex(&data)),
        "value": value_hex,
    });
    
    rpc.eth_estimate_gas(tx).await
}

/// 构建合约调用交易
/// 
/// # 参数
/// - `contract_address`: 合约地址
/// - `data`: 调用数据
/// - `chain_id`: 链 ID
/// - `nonce`: nonce
/// - `gas_limit`: Gas 限制
/// - `max_fee_per_gas_wei`: 最大费用（u128 Wei）
/// - `max_priority_fee_per_gas_wei`: 最大优先费用（u128 Wei）
/// 
/// # 返回
/// - Transaction 结构
pub fn build_contract_call(
    contract_address: &str,
    data: Vec<u8>,
    chain_id: u64,
    nonce: u64,
    gas_limit: u64,
    max_fee_per_gas_wei: u128,
    max_priority_fee_per_gas_wei: u128,
) -> Result<Transaction, String>
{
    TransactionBuilder::new()
        .tx_type(TxType::Eip1559)
        .to(contract_address)
        .data(data)
        .chain_id(chain_id)
        .nonce(nonce)
        .gas_limit(gas_limit)
        .max_fee_per_gas_u128(max_fee_per_gas_wei)
        .max_priority_fee_per_gas_u128(max_priority_fee_per_gas_wei)
        .build()
}

/// 构建合约部署交易
/// 
/// # 参数
/// - `bytecode`: 合约字节码
/// - `constructor_params`: 构造函数参数（已编码）
/// - `chain_id`: 链 ID
/// - `nonce`: nonce
/// - `gas_limit`: Gas 限制
/// - `max_fee_per_gas_wei`: 最大费用（u128 Wei）
/// - `max_priority_fee_per_gas_wei`: 最大优先费用（u128 Wei）
/// 
/// # 返回
/// - Transaction 结构
/// 
/// # 示例
/// ```
/// let bytecode = utils::hex_to_bytes("0x6080...")?;
/// let tx = build_deploy_transaction(
///     bytecode, 
///     vec![], 
///     1, 
///     0, 
///     5000000, 
///     50_000_000_000u128,  // 50 Gwei
///     2_000_000_000u128    // 2 Gwei
/// )?;
/// ```
pub fn build_deploy_transaction(
    bytecode: Vec<u8>,
    constructor_params: Vec<u8>,
    chain_id: u64,
    nonce: u64,
    gas_limit: u64,
    max_fee_per_gas_wei: u128,
    max_priority_fee_per_gas_wei: u128,
) -> Result<Transaction, String>
{
    let mut data = bytecode;
    data.extend_from_slice(&constructor_params);

    TransactionBuilder::new()
        .tx_type(TxType::Eip1559)
        .data(data)
        .chain_id(chain_id)
        .nonce(nonce)
        .gas_limit(gas_limit)
        .max_fee_per_gas_u128(max_fee_per_gas_wei)
        .max_priority_fee_per_gas_u128(max_priority_fee_per_gas_wei)
        .build()
}

/// ERC20 代币信息
#[derive(Debug, Clone)]
pub struct Erc20TokenInfo
{
    /// 代币名称
    pub name: String,
    /// 代币符号
    pub symbol: String,
    /// 精度
    pub decimals: u8,
    /// 总供应量（十六进制字符串）
    pub total_supply: String,
}

/// 获取 ERC20 代币信息
/// 
/// # 参数
/// - `rpc`: RPC 客户端
/// - `token_address`: 代币合约地址
/// 
/// # 返回
/// - Erc20TokenInfo 结构
pub async fn get_erc20_info(rpc: &RpcClient, token_address: &str) -> Result<Erc20TokenInfo, String>
{
    // 获取 name
    let name_data = encode_function_call("name()", vec![])?;
    let name_result = call_contract(rpc, token_address, name_data).await?;
    let name_tokens = decode_output(&name_result, vec![ParamType::String])?;
    let name = match &name_tokens[0]
    {
        Token::String(s) => s.clone(),
        _ => "Unknown".to_string(),
    };

    // 获取 symbol
    let symbol_data = encode_function_call("symbol()", vec![])?;
    let symbol_result = call_contract(rpc, token_address, symbol_data).await?;
    let symbol_tokens = decode_output(&symbol_result, vec![ParamType::String])?;
    let symbol = match &symbol_tokens[0]
    {
        Token::String(s) => s.clone(),
        _ => "???".to_string(),
    };

    // 获取 decimals
    let decimals_data = encode_function_call("decimals()", vec![])?;
    let decimals_result = call_contract(rpc, token_address, decimals_data).await?;
    let decimals_tokens = decode_output(&decimals_result, vec![ParamType::Uint(8)])?;    let decimals = match &decimals_tokens[0]
    {
        Token::Uint(v) =>
        {
            let bytes32 = utils::hex_to_bytes32(v)?;
            utils::bytes32_to_u64(&bytes32)? as u8
        }
        _ => 18,
    };

    // 获取 totalSupply
    let supply_data = encode_function_call("totalSupply()", vec![])?;
    let supply_result = call_contract(rpc, token_address, supply_data).await?;
    let supply_tokens = decode_output(&supply_result, vec![ParamType::Uint(256)])?;
    let total_supply = match &supply_tokens[0]
    {
        Token::Uint(v) => v.clone(),
        _ => "0x0".to_string(),
    };

    Ok(Erc20TokenInfo {
        name,
        symbol,
        decimals,
        total_supply,
    })
}

/// 获取 ERC20 代币余额
/// 
/// # 参数
/// - `rpc`: RPC 客户端
/// - `token_address`: 代币合约地址
/// - `owner`: 持有者地址
/// 
/// # 返回
/// - 代币余额（十六进制字符串）
pub async fn get_erc20_balance(
    rpc: &RpcClient,
    token_address: &str,
    owner: &str,
) -> Result<String, String>
{
    let data = encode_function_call(
        "balanceOf(address)",
        vec![Token::Address(utils::format_address(owner))],
    )?;
    
    let result = call_contract(rpc, token_address, data).await?;
    let tokens = decode_output(&result, vec![ParamType::Uint(256)])?;
    
    match &tokens[0]
    {
        Token::Uint(v) => Ok(v.clone()),
        _ => Err("Invalid balance response".into()),
    }
}

/// 编码 ERC20 transfer 调用数据
/// 
/// # 参数
/// - `to`: 接收者地址
/// - `amount_hex`: 转账数量（十六进制字符串）
/// 
/// # 返回
/// - 编码后的调用数据
pub fn encode_erc20_transfer(to: &str, amount_hex: &str) -> Result<Vec<u8>, String>
{
    encode_function_call(
        "transfer(address,uint256)",
        vec![
            Token::Address(utils::format_address(to)),
            Token::Uint(amount_hex.to_string()),
        ],
    )
}

/// 编码 ERC20 approve 调用数据
/// 
/// # 参数
/// - `spender`: 被授权者地址
/// - `amount_hex`: 授权数量（十六进制字符串）
/// 
/// # 返回
/// - 编码后的调用数据
pub fn encode_erc20_approve(spender: &str, amount_hex: &str) -> Result<Vec<u8>, String>
{
    encode_function_call(
        "approve(address,uint256)",
        vec![
            Token::Address(utils::format_address(spender)),
            Token::Uint(amount_hex.to_string()),
        ],
    )
}

/// 获取 ERC20 授权额度
/// 
/// # 参数
/// - `rpc`: RPC 客户端
/// - `token_address`: 代币合约地址
/// - `owner`: 持有者地址
/// - `spender`: 被授权者地址
/// 
/// # 返回
/// - 授权额度（十六进制字符串）
pub async fn get_erc20_allowance(
    rpc: &RpcClient,
    token_address: &str,
    owner: &str,
    spender: &str,
) -> Result<String, String>
{
    let data = encode_function_call(
        "allowance(address,address)",
        vec![
            Token::Address(utils::format_address(owner)),
            Token::Address(utils::format_address(spender)),
        ],
    )?;
    
    let result = call_contract(rpc, token_address, data).await?;
    let tokens = decode_output(&result, vec![ParamType::Uint(256)])?;
    
    match &tokens[0]
    {
        Token::Uint(v) => Ok(v.clone()),
        _ => Err("Invalid allowance response".into()),
    }
}
