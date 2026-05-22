use super::rpc::RpcClient;
use super::{contract, transaction, utils};

pub async fn send_native_transaction_units(
    from: &str,
    to: &str,
    amount_units: u128,
    sk_hex: &str,
    chain_id: u64,
    rpc_url: &str,
    is_max: bool,
) -> Result<String, String>
{
    let rpc = RpcClient::new(rpc_url);

    let nonce = rpc.eth_get_transaction_count(from, "latest").await?;

    let gas_price_hex = rpc.eth_gas_price().await?;
    let base_fee = utils::hex_to_u128(&gas_price_hex)?;

    let max_priority_fee = rpc
        .eth_max_priority_fee_per_gas()
        .await
        .ok()
        .and_then(|h| utils::hex_to_u128(&h).ok())
        .unwrap_or(1_000_000_000); // default 1 gwei
    let max_fee_per_gas = base_fee + base_fee / 4 + max_priority_fee;

    let amount_hex = format!("0x{:x}", amount_units);
    let estimate_tx = serde_json::json!({
        "from": from,
        "to": to,
        "value": amount_hex
    });
    let gas_estimate = rpc.eth_estimate_gas(estimate_tx).await.unwrap_or(21000);
    let gas_limit = gas_estimate + gas_estimate / 5;

    let mut final_amount = amount_units;
    if is_max
    {
        let l2_fee = max_fee_per_gas.saturating_mul(gas_limit as u128);
        let mut l1_fee = 0u128;

        if chain_id == 10 || chain_id == 8453
        {
            let dummy_tx = transaction::TransactionBuilder::new()
                .tx_type(transaction::TxType::Legacy)
                .chain_id(chain_id)
                .nonce(nonce)
                .to(to)
                .value_u128(amount_units)
                .gas_limit(gas_limit)
                .gas_price_u128(base_fee)
                .build()?;

            let dummy_signed = transaction::sign_transaction_with_hex(&dummy_tx, sk_hex)?;
            let dummy_rlp = transaction::serialize_signed_transaction(&dummy_signed);

            let oracle_data = utils::encode_get_l1_fee_data(&dummy_rlp);
            let call_req = serde_json::json!({
                "to": "0x420000000000000000000000000000000000000F",
                "data": oracle_data
            });
            if let Ok(l1_fee_hex) = rpc.eth_call(call_req, "latest").await
            {
                l1_fee = utils::hex_to_u128(&l1_fee_hex).unwrap_or(0);
            }
        }

        let total_fee = l2_fee.saturating_add(l1_fee);
        if amount_units <= total_fee
        {
            return Err("余额不足以支付网络手续费".to_string());
        }
        final_amount = amount_units - total_fee;
    }

    let tx = transaction::TransactionBuilder::new()
        .chain_id(chain_id)
        .nonce(nonce)
        .to(to)
        .value_u128(final_amount)
        .gas_limit(gas_limit)
        .max_fee_per_gas_u128(max_fee_per_gas)
        .max_priority_fee_per_gas_u128(max_priority_fee)
        .build()?;

    let signed = transaction::sign_transaction_with_hex(&tx, sk_hex)?;
    let raw_hex = transaction::serialize_signed_transaction(&signed);
    rpc.eth_send_raw_transaction(&raw_hex).await
}

pub async fn send_erc20_transaction(
    from: &str,
    to: &str,
    amount_units: u128,
    contract_address: &str,
    sk_hex: &str,
    chain_id: u64,
    rpc_url: &str,
) -> Result<String, String>
{
    let rpc = RpcClient::new(rpc_url);

    let nonce = rpc.eth_get_transaction_count(from, "latest").await?;

    let gas_price_hex = rpc.eth_gas_price().await?;
    let base_fee = utils::hex_to_u128(&gas_price_hex)?;

    let max_priority_fee = rpc
        .eth_max_priority_fee_per_gas()
        .await
        .ok()
        .and_then(|h| utils::hex_to_u128(&h).ok())
        .unwrap_or(1_000_000_000);
    let max_fee_per_gas = base_fee + base_fee / 4 + max_priority_fee;

    let amount_hex = format!("0x{:x}", amount_units);
    let data = contract::encode_erc20_transfer(to, &amount_hex)?;
    let data_hex = format!("0x{}", utils::bytes_to_hex(&data));

    let estimate_tx = serde_json::json!({
        "from": from,
        "to": contract_address,
        "data": data_hex
    });
    let gas_estimate = rpc.eth_estimate_gas(estimate_tx).await?;
    let gas_limit = gas_estimate + gas_estimate / 5;

    let tx = transaction::TransactionBuilder::new()
        .chain_id(chain_id)
        .nonce(nonce)
        .to(contract_address)
        .value_u128(0)
        .data(data)
        .gas_limit(gas_limit)
        .max_fee_per_gas_u128(max_fee_per_gas)
        .max_priority_fee_per_gas_u128(max_priority_fee)
        .build()?;

    let signed = transaction::sign_transaction_with_hex(&tx, sk_hex)?;
    let raw_hex = transaction::serialize_signed_transaction(&signed);
    rpc.eth_send_raw_transaction(&raw_hex).await
}
