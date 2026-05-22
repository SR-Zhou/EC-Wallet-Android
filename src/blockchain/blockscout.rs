//! Blockscout v2 API 模块
//! 提供按地址查询交易历史、代币转移记录等功能
//! 无需 API Key，所有 Blockscout 实例均免费开放

use serde::Deserialize;

pub struct BlockscoutClient {
    base_url: String,
    client: reqwest::Client,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockScoutTx {
    pub hash: String,
    pub from: AddressObj,
    pub to: Option<AddressObj>,
    pub value: String,
    pub gas_used: String,
    pub gas_price: String,
    pub gas_limit: String,
    pub result: String,
    pub timestamp: String,
    pub block_number: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddressObj {
    #[serde(alias = "hash")]
    pub hash: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockScoutTokenTx {
    pub transaction_hash: String,
    pub from: AddressObj,
    pub to: AddressObj,
    pub token: TokenInfo,
    pub total: TotalAmount,
    #[serde(default)]
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenInfo {
    #[serde(alias = "address_hash")]
    pub address_hash: String,
    pub symbol: String,
    pub decimals: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TotalAmount {
    pub value: String,
}

#[derive(Deserialize)]
struct TxListResponse {
    items: Vec<BlockScoutTx>,
}

#[derive(Deserialize)]
struct TokenTxListResponse {
    items: Vec<BlockScoutTokenTx>,
}

impl BlockscoutClient {
    pub fn new(base_url: &str) -> Self {
        BlockscoutClient {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(16))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    pub async fn get_transactions(&self, address: &str, count: u32) -> Result<Vec<BlockScoutTx>, String> {
        let url = format!(
            "{}/addresses/{}/transactions",
            self.base_url, address
        );
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Blockscout request failed: {}", e))?;
        let response = response
            .error_for_status()
            .map_err(|e| format!("Blockscout transaction response failed: {}", e))?;

        let api_response: TxListResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Blockscout response: {}", e))?;

        Ok(api_response.items.into_iter().take(count as usize).collect())
    }

    pub async fn get_token_transfers(&self, address: &str, count: u32) -> Result<Vec<BlockScoutTokenTx>, String> {
        let url = format!(
            "{}/addresses/{}/token-transfers",
            self.base_url, address
        );
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Blockscout token-transfers request failed: {}", e))?;
        let response = response
            .error_for_status()
            .map_err(|e| format!("Blockscout token-transfers response failed: {}", e))?;

        let api_response: TokenTxListResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Blockscout token-transfers response: {}", e))?;

        Ok(api_response.items.into_iter().take(count as usize).collect())
    }
}
