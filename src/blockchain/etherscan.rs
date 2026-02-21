//! Etherscan API 模块
//! 提供通过 Etherscan API 获取交易历史等功能
//! 标准 JSON-RPC 不支持按地址查询交易历史，需要使用 Etherscan API

use serde::Deserialize;

/// Etherscan API 客户端
pub struct EtherscanClient
{
    api_url: String,
    api_key: String,
    chain_id: u64,
    client: reqwest::Client,
}

/// 交易记录结构
#[derive(Debug, Clone, Deserialize)]
pub struct EtherscanTx
{
    /// 区块号
    #[serde(rename = "blockNumber")]
    pub block_number: String,
    /// 时间戳（Unix）
    #[serde(rename = "timeStamp")]
    pub timestamp: String,
    /// 交易哈希
    pub hash: String,
    /// 发送者
    pub from: String,
    /// 接收者
    pub to: String,
    /// 转账金额（Wei，十进制字符串）
    pub value: String,
    /// Gas 使用量
    #[serde(rename = "gasUsed")]
    pub gas_used: String,
    /// 是否出错 ("0" = 成功, "1" = 失败)
    #[serde(rename = "isError")]
    pub is_error: String,
}

/// Etherscan API 响应
#[derive(Deserialize)]
struct EtherscanResponse
{
    status: String,
    message: String,
    result: serde_json::Value,
}

impl EtherscanClient
{    /// 创建新的 Etherscan 客户端
    ///
    /// # 参数
    /// - `api_url`: Etherscan API 基础 URL（如 "https://api.etherscan.io"）
    /// - `api_key`: Etherscan API Key（空字符串将使用无 key 模式，速率受限）
    /// - `chain_id`: 链 ID（如 1 为 Ethereum 主网）
    pub fn new(api_url: &str, api_key: &str, chain_id: u64) -> Self
    {
        EtherscanClient {
            api_url: api_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            chain_id,
            client: reqwest::Client::new(),
        }
    }

    /// 获取地址的普通交易历史
    ///
    /// # 参数
    /// - `address`: 以太坊地址
    /// - `start_block`: 起始区块号（0 表示从创世区块开始）
    /// - `end_block`: 结束区块号（99999999 表示到最新）
    /// - `page`: 页码（从 1 开始）
    /// - `offset`: 每页数量（最多 10000）
    /// - `sort`: 排序方式（"asc" 或 "desc"）
    ///
    /// # 返回
    /// - 交易记录列表
    pub async fn get_normal_transactions(
        &self,
        address: &str,
        start_block: u64,
        end_block: u64,
        page: u32,
        offset: u32,
        sort: &str,
    ) -> Result<Vec<EtherscanTx>, String>
    {        let url = format!(
            "{}/v2/api?chainid={}&module=account&action=txlist&address={}&startblock={}&endblock={}&page={}&offset={}&sort={}&apikey={}",
            self.api_url, self.chain_id, address, start_block, end_block, page, offset, sort, self.api_key
        );

        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Etherscan request failed: {}", e))?;

        let api_response: EtherscanResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Etherscan response: {}", e))?;

        // status "0" 可能表示无结果（"No transactions found"）或错误
        if api_response.status == "0"
        {
            if api_response.message.contains("No transactions found")
            {
                return Ok(Vec::new());
            }
            // 如果 result 是字符串类型的错误信息
            if let Some(err_msg) = api_response.result.as_str()
            {
                return Err(format!("Etherscan API error: {}", err_msg));
            }
            return Ok(Vec::new());
        }

        let txs: Vec<EtherscanTx> = serde_json::from_value(api_response.result)
            .map_err(|e| format!("Failed to parse transactions: {}", e))?;

        Ok(txs)
    }

    /// 获取地址最近的交易历史（简化接口）
    ///
    /// # 参数
    /// - `address`: 以太坊地址
    /// - `count`: 获取数量（最多 10000）
    ///
    /// # 返回
    /// - 交易记录列表（按时间倒序）
    pub async fn get_recent_transactions(
        &self,
        address: &str,
        count: u32,
    ) -> Result<Vec<EtherscanTx>, String>
    {
        self.get_normal_transactions(address, 0, 99999999, 1, count, "desc").await
    }
}
