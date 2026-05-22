use super::{blockscout::BlockscoutClient, contract, rpc::RpcClient, utils};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxRecord {
    pub hash: String,
    pub from: String,
    pub to: String,
    pub timestamp: String,
    pub block_number: String,
    pub is_outgoing: bool,
    pub is_error: bool,
    pub gas_used: String,
    pub gas_price: String,
    pub gas_limit: String,
    pub token_type: String,
    pub value_wei: String,
    pub value_display: String,
    pub token_symbol: String,
    pub token_amount: String,
    pub token_decimals: u8,
    pub token_contract: String,
    pub chain_id: u64,
}

#[derive(Debug, Clone)]
pub struct TransactionQuery {
    pub chain_name: String,
    pub blockscout_api_url: Option<String>,
    pub usdc_address: Option<String>,
    pub usdt_address: Option<String>,
}

pub async fn fetch_balance(
    address: &str,
    rpc_url: &str,
    contract_address: Option<&str>,
    decimals: u8,
) -> Result<String, String> {
    let balance_units = fetch_balance_units(address, rpc_url, contract_address).await?;

    if balance_units == 0 {
        Ok("0".to_string())
    } else {
        Ok(format_units_to_decimal(balance_units, decimals, None))
    }
}

pub async fn fetch_balance_units(
    address: &str,
    rpc_url: &str,
    contract_address: Option<&str>,
) -> Result<u128, String> {
    let rpc = RpcClient::new(rpc_url);

    let balance_hex = match contract_address {
        None => rpc.eth_get_balance(address, "latest").await?,
        Some(contract_addr) => contract::get_erc20_balance(&rpc, contract_addr, address).await?,
    };

    utils::hex_to_u128(&balance_hex)
}

pub async fn fetch_transactions(
    address: &str,
    query: &TransactionQuery,
    chain_id: u64,
) -> Result<(Vec<TxRecord>, Vec<TxRecord>, Vec<TxRecord>), String> {
    let base_url = query
        .blockscout_api_url
        .as_deref()
        .ok_or_else(|| format!("{} 暂不支持交易历史查询", query.chain_name))?;

    let client = BlockscoutClient::new(base_url);
    let txs = client.get_transactions(address, 20).await?;
    let token_txs = client
        .get_token_transfers(address, 20)
        .await
        .unwrap_or_default();

    let my_addr = address.to_lowercase();
    let usdc_contract = query.usdc_address.as_ref().map(|a| a.to_lowercase());
    let usdt_contract = query.usdt_address.as_ref().map(|a| a.to_lowercase());

    let native = txs
        .iter()
        .filter(|tx| tx.value != "0")
        .map(|tx| {
            let is_outgoing = tx.from.hash.to_lowercase() == my_addr;
            TxRecord {
                hash: tx.hash.clone(),
                from: tx.from.hash.clone(),
                to: tx.to.as_ref().map(|to| to.hash.clone()).unwrap_or_default(),
                timestamp: format_iso_timestamp(&tx.timestamp),
                block_number: tx.block_number.to_string(),
                is_outgoing,
                is_error: tx.result != "success",
                gas_used: tx.gas_used.clone(),
                gas_price: tx.gas_price.clone(),
                gas_limit: tx.gas_limit.clone(),
                token_type: "Native".to_string(),
                value_wei: tx.value.clone(),
                value_display: wei_decimal_to_eth(&tx.value),
                token_symbol: String::new(),
                token_amount: String::new(),
                token_decimals: 0,
                token_contract: String::new(),
                chain_id,
            }
        })
        .collect();

    let mut usdc_txs = Vec::new();
    let mut usdt_txs = Vec::new();

    for tt in &token_txs {
        let token_addr = tt.token.address_hash.to_lowercase();
        let is_outgoing = tt.from.hash.to_lowercase() == my_addr;
        let decimals = tt.token.decimals.parse().unwrap_or(18);
        let amount_display = format_units_to_decimal_str(&tt.total.value, decimals);

        let record = TxRecord {
            hash: tt.transaction_hash.clone(),
            from: tt.from.hash.clone(),
            to: tt.to.hash.clone(),
            timestamp: tt
                .timestamp
                .as_deref()
                .map(format_iso_timestamp)
                .unwrap_or_default(),
            block_number: String::new(),
            is_outgoing,
            is_error: false,
            gas_used: String::new(),
            gas_price: String::new(),
            gas_limit: String::new(),
            token_type: "Token".to_string(),
            value_wei: String::new(),
            value_display: String::new(),
            token_symbol: tt.token.symbol.clone(),
            token_amount: amount_display,
            token_decimals: decimals,
            token_contract: tt.token.address_hash.clone(),
            chain_id,
        };

        if Some(&token_addr) == usdc_contract.as_ref() {
            usdc_txs.push(record);
        } else if Some(&token_addr) == usdt_contract.as_ref() {
            usdt_txs.push(record);
        }
    }

    Ok((native, usdc_txs, usdt_txs))
}

pub fn format_units_to_decimal(units: u128, decimals: u8, max_decimals: Option<usize>) -> String {
    let divisor = 10u128.pow(decimals as u32);
    let whole = units / divisor;
    let frac = units % divisor;

    if frac == 0 {
        format!("{}", whole)
    } else {
        let frac_str = format!("{:0>width$}", frac, width = decimals as usize);
        let mut trimmed = frac_str.trim_end_matches('0').to_string();
        if let Some(max) = max_decimals {
            if trimmed.len() > max {
                trimmed.truncate(max);
            }
        }
        if trimmed.is_empty() {
            format!("{}", whole)
        } else {
            format!("{}.{}", whole, trimmed)
        }
    }
}

fn wei_decimal_to_eth(wei_str: &str) -> String {
    if let Ok(wei) = wei_str.parse::<u128>() {
        let eth = utils::wei_to_ether(wei);
        if eth == 0.0 {
            "0".to_string()
        } else if eth < 0.0001 {
            format!("{:.6}", eth)
        } else {
            format!("{:.4}", eth)
        }
    } else {
        "0".to_string()
    }
}

fn format_iso_timestamp(iso_str: &str) -> String {
    if iso_str.len() >= 10 {
        let year: i32 = iso_str[0..4].parse().unwrap_or(1970);
        let month: i32 = iso_str[5..7].parse().unwrap_or(1);
        let day: i32 = iso_str[8..10].parse().unwrap_or(1);
        let hour: i32 = if iso_str.len() >= 13 {
            iso_str[11..13].parse().unwrap_or(0)
        } else {
            0
        };
        let minute: i32 = if iso_str.len() >= 16 {
            iso_str[14..16].parse().unwrap_or(0)
        } else {
            0
        };
        let utc_hour = (hour + 8) % 24;
        let extra_day = (hour + 8) / 24;
        let mut d = day + extra_day;
        let mut m = month;
        let mut y = year;
        while d > days_in_month(y, m) {
            d -= days_in_month(y, m);
            m += 1;
            if m > 12 {
                m = 1;
                y += 1;
            }
        }
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02} (UTC+8)",
            y,
            m,
            d,
            utc_hour,
            minute
        )
    } else {
        iso_str.to_string()
    }
}

fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn format_units_to_decimal_str(value_str: &str, decimals: u8) -> String {
    if decimals == 0 {
        return value_str.to_string();
    }
    if let Ok(value) = value_str.parse::<u128>() {
        let divisor = 10u128.pow(decimals as u32);
        let whole = value / divisor;
        let frac = value % divisor;
        if frac == 0 {
            return whole.to_string();
        }
        let frac_str = format!("{:0>width$}", frac, width = decimals as usize);
        let trimmed = frac_str.trim_end_matches('0');
        format!("{}.{}", whole, trimmed)
    } else {
        value_str.to_string()
    }
}
