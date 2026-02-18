//! RPC 客户端模块
//! 负责与 EVM 链节点通信，封装所有 JSON-RPC 调用

use super::utils;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

/// RPC 客户端
pub struct RpcClient
{
    url: String,
    client: reqwest::Client,
    request_id: AtomicU64,
}

/// JSON-RPC 请求结构
#[derive(Serialize)]
struct JsonRpcRequest
{
    jsonrpc: String,
    method: String,
    params: Value,
    id: u64,
}

/// JSON-RPC 响应结构
#[derive(Deserialize)]
struct JsonRpcResponse
{
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

/// JSON-RPC 错误结构
#[derive(Deserialize)]
struct JsonRpcError
{
    code: i64,
    message: String,
}

impl RpcClient
{
    /// 创建新的 RPC 客户端
    /// 
    /// # 参数
    /// - `url`: RPC 节点 URL
    /// 
    /// # 返回
    /// - RpcClient 实例
    /// 
    /// # 示例
    /// ```
    /// let client = RpcClient::new("https://eth-mainnet.public.blastapi.io");
    /// ```
    pub fn new(url: &str) -> Self
    {
        RpcClient {
            url: url.to_string(),
            client: reqwest::Client::new(),
            request_id: AtomicU64::new(1),
        }
    }

    /// 发送原始 JSON-RPC 请求
    /// 
    /// # 参数
    /// - `method`: RPC 方法名
    /// - `params`: 参数列表
    /// 
    /// # 返回
    /// - JSON 响应值
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, String>
    {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id,
        };

        let response = self.client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("RPC request failed: {}", e))?;

        let response_text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read RPC response: {}", e))?;

        let json_response: JsonRpcResponse = serde_json::from_str(&response_text)
            .map_err(|_| {
                // 如果无法解析为标准 JSON-RPC 响应，尝试提取错误信息
                if let Ok(v) = serde_json::from_str::<Value>(&response_text) {
                    if let Some(err) = v.get("error") {
                        if let Some(msg) = err.as_str() {
                            return format!("RPC error: {}", msg);
                        }
                        if let Some(msg) = err.get("message").and_then(|m| m.as_str()) {
                            return format!("RPC error: {}", msg);
                        }
                    }
                }
                format!("Failed to parse RPC response: {}", response_text.chars().take(200).collect::<String>())
            })?;

        if let Some(error) = json_response.error
        {
            return Err(format!("RPC error {}: {}", error.code, error.message));
        }

        json_response.result.ok_or_else(|| "Empty RPC response".to_string())
    }

    /// 获取链 ID
    /// 
    /// # 返回
    /// - 链 ID (u64)
    pub async fn eth_chain_id(&self) -> Result<u64, String>
    {
        let result = self.call("eth_chainId", json!([])).await?;
        let hex = result.as_str().ok_or("Invalid chainId response")?;
        utils::hex_to_u64(hex)
    }

    /// 获取当前区块号
    /// 
    /// # 返回
    /// - 区块号 (u64)
    pub async fn eth_block_number(&self) -> Result<u64, String>
    {
        let result = self.call("eth_blockNumber", json!([])).await?;
        let hex = result.as_str().ok_or("Invalid blockNumber response")?;
        utils::hex_to_u64(hex)
    }

    /// 获取账户余额
    /// 
    /// # 参数
    /// - `address`: 账户地址
    /// - `block`: 区块标识（"latest", "pending", 或区块号）
    /// 
    /// # 返回
    /// - 余额的十六进制字符串
    pub async fn eth_get_balance(&self, address: &str, block: &str) -> Result<String, String>
    {
        let result = self.call("eth_getBalance", json!([address, block])).await?;
        result.as_str()
            .map(|s| s.to_string())
            .ok_or("Invalid balance response".to_string())
    }

    /// 获取账户 nonce（交易计数）
    /// 
    /// # 参数
    /// - `address`: 账户地址
    /// - `block`: 区块标识
    /// 
    /// # 返回
    /// - nonce (u64)
    pub async fn eth_get_transaction_count(&self, address: &str, block: &str) -> Result<u64, String>
    {
        let result = self.call("eth_getTransactionCount", json!([address, block])).await?;
        let hex = result.as_str().ok_or("Invalid transactionCount response")?;
        utils::hex_to_u64(hex)
    }

    /// 发送已签名的原始交易
    /// 
    /// # 参数
    /// - `signed_tx`: 已签名交易的十六进制字符串（带 0x 前缀）
    /// 
    /// # 返回
    /// - 交易哈希
    pub async fn eth_send_raw_transaction(&self, signed_tx: &str) -> Result<String, String>
    {
        let result = self.call("eth_sendRawTransaction", json!([signed_tx])).await?;
        result.as_str()
            .map(|s| s.to_string())
            .ok_or("Invalid sendRawTransaction response".to_string())
    }

    /// 获取交易回执
    /// 
    /// # 参数
    /// - `tx_hash`: 交易哈希
    /// 
    /// # 返回
    /// - 交易回执 JSON（如果交易未确认则返回 None）
    pub async fn eth_get_transaction_receipt(&self, tx_hash: &str) -> Result<Option<Value>, String>
    {
        let result = self.call("eth_getTransactionReceipt", json!([tx_hash])).await?;
        if result.is_null()
        {
            Ok(None)
        }
        else
        {
            Ok(Some(result))
        }
    }

    /// 估算交易 Gas
    /// 
    /// # 参数
    /// - `tx`: 交易对象 JSON
    /// 
    /// # 返回
    /// - 估算的 Gas 数量 (u64)
    pub async fn eth_estimate_gas(&self, tx: Value) -> Result<u64, String>
    {
        let result = self.call("eth_estimateGas", json!([tx])).await?;
        let hex = result.as_str().ok_or("Invalid estimateGas response")?;
        utils::hex_to_u64(hex)
    }

    /// 获取当前 Gas 价格（Legacy 交易）
    /// 
    /// # 返回
    /// - Gas 价格的十六进制字符串
    pub async fn eth_gas_price(&self) -> Result<String, String>
    {
        let result = self.call("eth_gasPrice", json!([])).await?;
        result.as_str()
            .map(|s| s.to_string())
            .ok_or("Invalid gasPrice response".to_string())
    }

    /// 获取建议的优先费用（EIP-1559 交易）
    /// 
    /// # 返回
    /// - 优先费用的十六进制字符串
    pub async fn eth_max_priority_fee_per_gas(&self) -> Result<String, String>
    {
        let result = self.call("eth_maxPriorityFeePerGas", json!([])).await?;
        result.as_str()
            .map(|s| s.to_string())
            .ok_or("Invalid maxPriorityFeePerGas response".to_string())
    }

    /// 调用合约（不发送交易，只读）
    /// 
    /// # 参数
    /// - `tx`: 调用对象 JSON
    /// - `block`: 区块标识
    /// 
    /// # 返回
    /// - 调用结果的十六进制字符串
    pub async fn eth_call(&self, tx: Value, block: &str) -> Result<String, String>
    {
        let result = self.call("eth_call", json!([tx, block])).await?;
        result.as_str()
            .map(|s| s.to_string())
            .ok_or("Invalid eth_call response".to_string())
    }

    /// 获取区块信息
    /// 
    /// # 参数
    /// - `block`: 区块号的十六进制字符串
    /// - `full_transactions`: 是否包含完整交易信息
    /// 
    /// # 返回
    /// - 区块信息 JSON
    pub async fn eth_get_block_by_number(&self, block: &str, full_transactions: bool) -> Result<Option<Value>, String>
    {
        let result = self.call("eth_getBlockByNumber", json!([block, full_transactions])).await?;
        if result.is_null()
        {
            Ok(None)
        }
        else
        {
            Ok(Some(result))
        }
    }

    /// 通过哈希获取区块信息
    /// 
    /// # 参数
    /// - `block_hash`: 区块哈希
    /// - `full_transactions`: 是否包含完整交易信息
    /// 
    /// # 返回
    /// - 区块信息 JSON
    pub async fn eth_get_block_by_hash(&self, block_hash: &str, full_transactions: bool) -> Result<Option<Value>, String>
    {
        let result = self.call("eth_getBlockByHash", json!([block_hash, full_transactions])).await?;
        if result.is_null()
        {
            Ok(None)
        }
        else
        {
            Ok(Some(result))
        }
    }

    /// 获取交易信息
    /// 
    /// # 参数
    /// - `tx_hash`: 交易哈希
    /// 
    /// # 返回
    /// - 交易信息 JSON
    pub async fn eth_get_transaction_by_hash(&self, tx_hash: &str) -> Result<Option<Value>, String>
    {
        let result = self.call("eth_getTransactionByHash", json!([tx_hash])).await?;
        if result.is_null()
        {
            Ok(None)
        }
        else
        {
            Ok(Some(result))
        }
    }

    /// 获取事件日志
    /// 
    /// # 参数
    /// - `filter`: 日志过滤器 JSON
    /// 
    /// # 返回
    /// - 日志列表 JSON
    pub async fn eth_get_logs(&self, filter: Value) -> Result<Value, String>
    {
        self.call("eth_getLogs", json!([filter])).await
    }

    /// 获取合约代码
    /// 
    /// # 参数
    /// - `address`: 合约地址
    /// - `block`: 区块标识
    /// 
    /// # 返回
    /// - 合约字节码的十六进制字符串
    pub async fn eth_get_code(&self, address: &str, block: &str) -> Result<String, String>
    {
        let result = self.call("eth_getCode", json!([address, block])).await?;
        result.as_str()
            .map(|s| s.to_string())
            .ok_or("Invalid getCode response".to_string())
    }    /// 获取最新区块的基础费用
    /// 
    /// # 返回
    /// - 基础费用（十六进制字符串）
    pub async fn get_base_fee(&self) -> Result<String, String>
    {
        let block = self.eth_get_block_by_number("latest", false).await?;
        let block = block.ok_or("Failed to get latest block")?;
        let base_fee_hex = block["baseFeePerGas"]
            .as_str()
            .ok_or("Block does not contain baseFeePerGas (pre-EIP-1559 chain?)")?;
        Ok(base_fee_hex.to_string())
    }
}
