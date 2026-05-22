//! Uniswap V3 交换模块
//! 使用 Uniswap V3 SwapRouter02 和 Quoter V2 实现代币交换
//! 无需 API Key，直接与链上合约交互

use super::contract;
use super::rpc::RpcClient;
use super::transaction::{self, TransactionBuilder};
use super::utils;
use serde_json::json;

// ============ 合约地址 ============

/// Uniswap V3 SwapRouter02 地址（多链通用）
const SWAP_ROUTER_02: &str = "0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45";

/// Uniswap V3 Quoter V2 地址（多链通用）
const QUOTER_V2: &str = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e";

/// WETH9 地址（各链）
/// 返回指定链的 WETH 地址，不支持的链返回 None
pub fn weth_address(chain_id: u64) -> Option<&'static str>
{
    match chain_id
    {
        1 => Some("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),       // Ethereum
        42161 => Some("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1"),   // Arbitrum
        10 => Some("0x4200000000000000000000000000000000000006"),       // Optimism
        8453 => Some("0x4200000000000000000000000000000000000006"),     // Base
        137 => Some("0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270"),     // Polygon (WMATIC)
        56 => Some("0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c"),      // BSC (WBNB)
        _ => None,
    }
}

/// 检查链是否支持 Uniswap V3
pub fn is_chain_supported(chain_id: u64) -> bool
{
    matches!(chain_id, 1 | 42161 | 10 | 8453 | 137)
}

// ============ 报价 ============

#[allow(dead_code)]
/// 交换报价结果
pub struct SwapQuote
{
    /// 预计输出数量（最小单位）
    pub amount_out: u128,
    /// 预计输出数量（十六进制）
    pub amount_out_hex: String,
    /// 预计 Gas 费用
    pub estimated_gas: u128,
}

/// 获取交换报价
/// 
/// 使用 Uniswap V3 Quoter V2 的 quoteExactInputSingle 函数
///
/// # 参数
/// - `rpc`: RPC 客户端
/// - `token_in`: 输入代币地址（原生代币用 WETH 地址）
/// - `token_out`: 输出代币地址（原生代币用 WETH 地址）
/// - `amount_in`: 输入数量（最小单位的十六进制）
/// - `fee`: 池子费率（500 = 0.05%, 3000 = 0.3%, 10000 = 1%）
///
/// # 返回
/// - 交换报价
pub async fn get_quote(
    rpc: &RpcClient,
    token_in: &str,
    token_out: &str,
    amount_in: &str,
    fee: u32,
) -> Result<SwapQuote, String>
{
    // quoteExactInputSingle((address,address,uint256,uint24,uint160))
    // 参数是一个元组结构体 QuoteExactInputSingleParams
    // tokenIn, tokenOut, amountIn, fee, sqrtPriceLimitX96
    //
    // 但 Quoter V2 的实际签名为：
    // quoteExactInputSingle(address tokenIn, address tokenOut, uint256 amountIn, uint24 fee, uint160 sqrtPriceLimitX96)
    // 返回：(uint256 amountOut, uint160 sqrtPriceX96After, uint32 initializedTicksCrossed, uint256 gasEstimate)

    // 手动编码调用数据，因为需要编码 tuple
    let selector = utils::function_selector(
        "quoteExactInputSingle((address,address,uint256,uint24,uint160))"
    );

    // 编码参数为 tuple（作为一个整体）
    let token_in_bytes = utils::hex_to_bytes(&utils::format_address(token_in))
        .map_err(|e| format!("Invalid token_in: {}", e))?;
    let token_out_bytes = utils::hex_to_bytes(&utils::format_address(token_out))
        .map_err(|e| format!("Invalid token_out: {}", e))?;
    let amount_in_bytes = utils::hex_to_bytes32(amount_in)?;
    let fee_bytes = utils::u64_to_bytes32(fee as u64);
    // sqrtPriceLimitX96 = 0（表示无限制）
    let sqrt_price_limit = [0u8; 32];

    let mut data = selector.to_vec();
    // tuple 内容
    data.extend_from_slice(&pad_left_32(&token_in_bytes));
    data.extend_from_slice(&pad_left_32(&token_out_bytes));
    data.extend_from_slice(&amount_in_bytes);
    data.extend_from_slice(&fee_bytes);
    data.extend_from_slice(&sqrt_price_limit);

    let data_hex = utils::add_0x_prefix(&utils::bytes_to_hex(&data));

    let tx = json!({
        "to": utils::format_address(QUOTER_V2),
        "data": data_hex,
    });

    let result = rpc.eth_call(tx, "latest").await?;

    // 解码返回值：(uint256 amountOut, uint160 sqrtPriceX96After, uint32 initializedTicksCrossed, uint256 gasEstimate)
    let result_bytes = utils::hex_to_bytes(&result)?;
    if result_bytes.len() < 128
    {
        return Err(format!("Quoter 返回数据太短 ({}字节)，可能该交易对不存在或流动性不足", result_bytes.len()));
    }

    let mut amount_out_arr = [0u8; 32];
    amount_out_arr.copy_from_slice(&result_bytes[0..32]);
    let amount_out_hex = utils::bytes32_to_hex(&amount_out_arr);
    let amount_out = utils::hex_to_u128(&amount_out_hex)?;

    let mut gas_arr = [0u8; 32];
    gas_arr.copy_from_slice(&result_bytes[96..128]);
    let gas_hex = utils::bytes32_to_hex(&gas_arr);
    let estimated_gas = utils::hex_to_u128(&gas_hex).unwrap_or(200_000);

    Ok(SwapQuote {
        amount_out,
        amount_out_hex,
        estimated_gas,
    })
}

pub async fn get_quote_with_rpc_url(
    rpc_url: &str,
    token_in: &str,
    token_out: &str,
    amount_in: &str,
    fee: u32,
) -> Result<SwapQuote, String>
{
    let rpc = RpcClient::new(rpc_url);
    get_quote(&rpc, token_in, token_out, amount_in, fee).await
}

// ============ 执行交换 ============

/// 交换参数
#[derive(Debug, Clone)]
pub struct SwapParams
{
    /// 输入代币地址（原生代币传 "native"）
    pub token_in: String,
    /// 输出代币地址（原生代币传 "native"）
    pub token_out: String,
    /// 输入数量（最小单位，u128）
    pub amount_in: u128,
    /// 最小输出数量（最小单位，u128）——滑点保护
    pub amount_out_min: u128,
    /// 池子费率（500, 3000, 10000）
    pub fee: u32,
    /// 发送者/接收者地址
    pub recipient: String,
    /// 链 ID
    pub chain_id: u64,
}

/// 执行交换交易
///
/// # 参数
/// - `rpc`: RPC 客户端
/// - `params`: 交换参数
/// - `sk_hex`: 私钥十六进制字符串
///
/// # 返回
/// - 交易哈希
pub async fn execute_swap(
    rpc: &RpcClient,
    params: &SwapParams,
    sk_hex: &str,
) -> Result<String, String>
{
    let weth = weth_address(params.chain_id)
        .ok_or_else(|| format!("不支持的链: {}", params.chain_id))?;

    let is_native_in = params.token_in == "native";
    let is_native_out = params.token_out == "native";

    let actual_token_in = if is_native_in { weth.to_string() } else { params.token_in.clone() };
    let actual_token_out = if is_native_out { weth.to_string() } else { params.token_out.clone() };

    // 如果输入不是原生代币，需要先检查并执行 approve
    if !is_native_in
    {
        check_and_approve(rpc, &params.token_in, SWAP_ROUTER_02, params.amount_in, &params.recipient, params.chain_id, sk_hex).await?;
    }

    // 构建 SwapRouter02 的 multicall 数据
    let swap_data = build_swap_calldata(
        &actual_token_in,
        &actual_token_out,
        params.amount_in,
        params.amount_out_min,
        params.fee,
        &params.recipient,
        is_native_in,
        is_native_out,
    )?;

    // 附带的 ETH 值（如果输入是原生代币）
    let value = if is_native_in { params.amount_in } else { 0u128 };

    // 获取 nonce
    let nonce = rpc.eth_get_transaction_count(&params.recipient, "latest").await?;

    // 获取 gas price
    let gas_price_hex = rpc.eth_gas_price().await?;
    let base_fee = utils::hex_to_u128(&gas_price_hex)?;

    let max_priority_fee = rpc
        .eth_max_priority_fee_per_gas()
        .await
        .ok()
        .and_then(|h| utils::hex_to_u128(&h).ok())
        .unwrap_or(1_000_000_000);
    let max_fee_per_gas = base_fee + base_fee / 4 + max_priority_fee;

    // 估算 gas
    let data_hex = utils::add_0x_prefix(&utils::bytes_to_hex(&swap_data));
    let value_hex = utils::u128_to_hex(value);
    let estimate_tx = json!({
        "from": utils::format_address(&params.recipient),
        "to": utils::format_address(SWAP_ROUTER_02),
        "data": data_hex,
        "value": value_hex,
    });
    let gas_estimate = rpc.eth_estimate_gas(estimate_tx).await
        .map_err(|e| format!("Gas 估算失败（可能流动性不足或滑点过大）: {}", e))?;
    let gas_limit = gas_estimate + gas_estimate / 5; // 加 20% 余量

    // 构建交易
    let tx = TransactionBuilder::new()
        .chain_id(params.chain_id)
        .nonce(nonce)
        .to(SWAP_ROUTER_02)
        .value_u128(value)
        .data(swap_data)
        .gas_limit(gas_limit)
        .max_fee_per_gas_u128(max_fee_per_gas)
        .max_priority_fee_per_gas_u128(max_priority_fee)
        .build()?;

    // 签名并发送
    let signed = transaction::sign_transaction_with_hex(&tx, sk_hex)?;
    let raw_hex = transaction::serialize_signed_transaction(&signed);
    let tx_hash = rpc.eth_send_raw_transaction(&raw_hex).await?;
    Ok(tx_hash)
}

pub async fn execute_swap_with_rpc_url(
    rpc_url: &str,
    params: &SwapParams,
    sk_hex: &str,
) -> Result<String, String>
{
    let rpc = RpcClient::new(rpc_url);
    execute_swap(&rpc, params, sk_hex).await
}

// ============ 内部辅助函数 ============

/// 构建交换 calldata
///
/// 对于 ETH -> Token: exactInputSingle + refundETH (via multicall)
/// 对于 Token -> ETH: exactInputSingle (recipient=router) + unwrapWETH9 (via multicall)
/// 对于 Token -> Token: exactInputSingle (直接调用)
fn build_swap_calldata(
    token_in: &str,
    token_out: &str,
    amount_in: u128,
    amount_out_min: u128,
    fee: u32,
    recipient: &str,
    is_native_in: bool,
    is_native_out: bool,
) -> Result<Vec<u8>, String>
{
    if is_native_in
    {
        // ETH -> Token: multicall([exactInputSingle, refundETH])
        let swap_call = encode_exact_input_single(
            token_in, token_out, fee, recipient, amount_in, amount_out_min,
        )?;
        let refund_call = encode_refund_eth()?;
        encode_multicall(&[&swap_call, &refund_call])
    }
    else if is_native_out
    {
        // Token -> ETH: multicall([exactInputSingle(recipient=router), unwrapWETH9])
        // recipient 设为 address(2) 表示 router 自身（SwapRouter02 约定）
        let router_as_recipient = "0x0000000000000000000000000000000000000002";
        let swap_call = encode_exact_input_single(
            token_in, token_out, fee, router_as_recipient, amount_in, amount_out_min,
        )?;
        let unwrap_call = encode_unwrap_weth9(amount_out_min, recipient)?;
        encode_multicall(&[&swap_call, &unwrap_call])
    }
    else
    {
        // Token -> Token: 直接调用 exactInputSingle
        encode_exact_input_single(
            token_in, token_out, fee, recipient, amount_in, amount_out_min,
        )
    }
}

/// 编码 exactInputSingle 调用
/// SwapRouter02 签名:
/// exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))
fn encode_exact_input_single(
    token_in: &str,
    token_out: &str,
    fee: u32,
    recipient: &str,
    amount_in: u128,
    amount_out_min: u128,
) -> Result<Vec<u8>, String>
{
    let selector = utils::function_selector(
        "exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))"
    );

    let token_in_bytes = utils::hex_to_bytes(&utils::format_address(token_in))?;
    let token_out_bytes = utils::hex_to_bytes(&utils::format_address(token_out))?;
    let recipient_bytes = utils::hex_to_bytes(&utils::format_address(recipient))?;

    let fee_bytes = utils::u64_to_bytes32(fee as u64);
    let amount_in_bytes = u128_to_bytes32(amount_in);
    let amount_out_min_bytes = u128_to_bytes32(amount_out_min);
    let sqrt_price_limit = [0u8; 32]; // 无限制

    let mut data = selector.to_vec();
    // tuple 内容: tokenIn, tokenOut, fee, recipient, amountIn, amountOutMinimum, sqrtPriceLimitX96
    data.extend_from_slice(&pad_left_32(&token_in_bytes));
    data.extend_from_slice(&pad_left_32(&token_out_bytes));
    data.extend_from_slice(&fee_bytes);
    data.extend_from_slice(&pad_left_32(&recipient_bytes));
    data.extend_from_slice(&amount_in_bytes);
    data.extend_from_slice(&amount_out_min_bytes);
    data.extend_from_slice(&sqrt_price_limit);

    Ok(data)
}

/// 编码 refundETH() 调用
fn encode_refund_eth() -> Result<Vec<u8>, String>
{
    let selector = utils::function_selector("refundETH()");
    Ok(selector.to_vec())
}

/// 编码 unwrapWETH9(uint256 amountMinimum, address recipient) 调用
fn encode_unwrap_weth9(amount_min: u128, recipient: &str) -> Result<Vec<u8>, String>
{
    let selector = utils::function_selector("unwrapWETH9(uint256,address)");
    let recipient_bytes = utils::hex_to_bytes(&utils::format_address(recipient))?;

    let mut data = selector.to_vec();
    data.extend_from_slice(&u128_to_bytes32(amount_min));
    data.extend_from_slice(&pad_left_32(&recipient_bytes));
    Ok(data)
}

/// 编码 multicall(bytes[]) 调用
/// SwapRouter02 的 multicall 签名: multicall(bytes[])
fn encode_multicall(calls: &[&[u8]]) -> Result<Vec<u8>, String>
{
    let selector = utils::function_selector("multicall(bytes[])");

    let mut data = selector.to_vec();

    // bytes[] 是动态类型，先写偏移量
    data.extend_from_slice(&utils::u64_to_bytes32(32)); // offset to array

    // 数组长度
    data.extend_from_slice(&utils::u64_to_bytes32(calls.len() as u64));

    // 每个元素的偏移量（相对于数组数据开始位置）
    // 数组数据开始位置 = 当前位置
    // 先写 N 个偏移量（每个32字节），然后写每个 bytes 的实际数据
    let offsets_size = calls.len() * 32;
    let mut current_offset = offsets_size;
    let mut encoded_elements = Vec::new();

    for call in calls
    {
        data.extend_from_slice(&utils::u64_to_bytes32(current_offset as u64));
        // 每个 bytes 编码为: length (32 bytes) + data (padded to 32)
        let mut element = Vec::new();
        element.extend_from_slice(&utils::u64_to_bytes32(call.len() as u64));
        element.extend_from_slice(&pad_right_32(call));
        current_offset += element.len();
        encoded_elements.extend_from_slice(&element);
    }

    data.extend_from_slice(&encoded_elements);
    Ok(data)
}

/// 检查并执行 ERC20 approve
async fn check_and_approve(
    rpc: &RpcClient,
    token: &str,
    spender: &str,
    amount: u128,
    owner: &str,
    chain_id: u64,
    sk_hex: &str,
) -> Result<(), String>
{
    // 查询当前 allowance
    let allowance_hex = contract::get_erc20_allowance(rpc, token, owner, spender).await?;
    let allowance = utils::hex_to_u128(&allowance_hex).unwrap_or(0);

    if allowance >= amount
    {
        return Ok(()); // 已有足够的授权
    }

    // 授权最大值 (type(uint256).max)
    let max_approve = "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    let data = contract::encode_erc20_approve(spender, max_approve)?;
    let data_hex = utils::add_0x_prefix(&utils::bytes_to_hex(&data));

    let nonce = rpc.eth_get_transaction_count(owner, "latest").await?;
    let gas_price_hex = rpc.eth_gas_price().await?;
    let base_fee = utils::hex_to_u128(&gas_price_hex)?;

    let max_priority_fee = rpc
        .eth_max_priority_fee_per_gas()
        .await
        .ok()
        .and_then(|h| utils::hex_to_u128(&h).ok())
        .unwrap_or(1_000_000_000);
    let max_fee_per_gas = base_fee + base_fee / 4 + max_priority_fee;

    // 估算 gas
    let estimate_tx = json!({
        "from": utils::format_address(owner),
        "to": utils::format_address(token),
        "data": data_hex,
    });
    let gas_estimate = rpc.eth_estimate_gas(estimate_tx).await?;
    let gas_limit = gas_estimate + gas_estimate / 5;

    let approve_data = contract::encode_erc20_approve(spender, max_approve)?;

    let tx = TransactionBuilder::new()
        .chain_id(chain_id)
        .nonce(nonce)
        .to(token)
        .value_u128(0)
        .data(approve_data)
        .gas_limit(gas_limit)
        .max_fee_per_gas_u128(max_fee_per_gas)
        .max_priority_fee_per_gas_u128(max_priority_fee)
        .build()?;

    let signed = transaction::sign_transaction_with_hex(&tx, sk_hex)?;
    let raw_hex = transaction::serialize_signed_transaction(&signed);
    let tx_hash = rpc.eth_send_raw_transaction(&raw_hex).await?;

    // 等待 approve 交易确认
    wait_for_confirmation(rpc, &tx_hash, 60).await?;

    Ok(())
}

/// 等待交易确认
async fn wait_for_confirmation(
    rpc: &RpcClient,
    tx_hash: &str,
    timeout_secs: u64,
) -> Result<(), String>
{
    let start = std::time::Instant::now();
    loop
    {
        if start.elapsed().as_secs() > timeout_secs
        {
            return Err("等待交易确认超时".to_string());
        }

        if let Ok(Some(receipt)) = rpc.eth_get_transaction_receipt(tx_hash).await
        {
            let status = receipt["status"].as_str().unwrap_or("0x0");
            if status == "0x1"
            {
                return Ok(());
            }
            else
            {
                return Err("交易执行失败".to_string());
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

/// 将 u128 转换为 32 字节大端序数组
fn u128_to_bytes32(value: u128) -> [u8; 32]
{
    let mut result = [0u8; 32];
    result[16..32].copy_from_slice(&value.to_be_bytes());
    result
}

/// 左填充到 32 字节
fn pad_left_32(data: &[u8]) -> [u8; 32]
{
    let mut result = [0u8; 32];
    let start = 32usize.saturating_sub(data.len());
    let copy_len = data.len().min(32);
    result[start..start + copy_len].copy_from_slice(&data[data.len() - copy_len..]);
    result
}

/// 右填充到 32 的倍数
fn pad_right_32(data: &[u8]) -> Vec<u8>
{
    let padded_len = ((data.len() + 31) / 32) * 32;
    let mut result = data.to_vec();
    result.resize(padded_len, 0);
    result
}

/// 格式化代币数量为可读字符串
///
/// # 参数
/// - `amount`: 最小单位数量
/// - `decimals`: 精度
///
/// # 返回
/// - 格式化后的字符串（如 "1.234567"）
pub fn format_token_amount(amount: u128, decimals: u8) -> String
{
    if decimals == 0
    {
        return amount.to_string();
    }

    let divisor = 10u128.pow(decimals as u32);
    let whole = amount / divisor;
    let frac = amount % divisor;

    if frac == 0
    {
        return whole.to_string();
    }

    let frac_str = format!("{:0>width$}", frac, width = decimals as usize);
    let trimmed = frac_str.trim_end_matches('0');
    format!("{}.{}", whole, trimmed)
}
