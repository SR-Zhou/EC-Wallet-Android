//! 交易构建模块
//! 负责构建、签名和序列化交易（包括 Legacy、EIP-1559 等类型）
//! 
//! 跨 crate 接口只暴露 Rust 标准类型（String、字节数组、基础整数类型）

use super::utils;
use super::conversion;
use k256::ecdsa::{SigningKey, recoverable, signature::Signer};
use k256::elliptic_curve::bigint::U256;
use k256::elliptic_curve::bigint::Encoding;
use k256::SecretKey;

/// 交易类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TxType
{
    /// Legacy 交易（EIP-155）
    Legacy,
    /// EIP-1559 交易
    Eip1559,
}

/// 交易结构
#[derive(Debug, Clone)]
pub struct Transaction
{
    /// 交易类型
    pub tx_type: TxType,
    /// 链 ID
    pub chain_id: u64,
    /// nonce
    pub nonce: u64,
    /// 接收地址（None 表示合约创建）
    pub to: Option<String>,
    /// 转账金额（Wei）
    pub value: U256,
    /// 交易数据
    pub data: Vec<u8>,
    /// Gas 限制
    pub gas_limit: u64,
    /// Gas 价格（Legacy 交易）
    pub gas_price: Option<U256>,
    /// 最大基础费用（EIP-1559）
    pub max_fee_per_gas: Option<U256>,
    /// 最大优先费用（EIP-1559）
    pub max_priority_fee_per_gas: Option<U256>,
}

/// 已签名交易结构
#[derive(Debug, Clone)]
pub struct SignedTransaction
{
    /// 原始交易
    pub transaction: Transaction,
    /// 签名 v 值
    pub v: u64,
    /// 签名 r 值
    pub r: U256,
    /// 签名 s 值
    pub s: U256,
}

/// 交易构建器
#[derive(Debug, Clone)]
pub struct TransactionBuilder
{
    tx_type: TxType,
    chain_id: Option<u64>,
    nonce: Option<u64>,
    to: Option<String>,
    value: U256,
    data: Vec<u8>,
    gas_limit: Option<u64>,
    gas_price: Option<U256>,
    max_fee_per_gas: Option<U256>,
    max_priority_fee_per_gas: Option<U256>,
}

impl Default for TransactionBuilder
{
    fn default() -> Self
    {
        Self::new()
    }
}

impl TransactionBuilder
{
    /// 创建新的交易构建器
    /// 
    /// # 返回
    /// - TransactionBuilder 实例
    /// 
    /// # 示例
    /// ```
    /// let tx = TransactionBuilder::new()
    ///     .to("0x...")
    ///     .value(ether_to_wei(0.1))
    ///     .build()?;
    /// ```
    pub fn new() -> Self
    {
        TransactionBuilder {
            tx_type: TxType::Eip1559,
            chain_id: None,
            nonce: None,
            to: None,
            value: U256::ZERO,
            data: Vec::new(),
            gas_limit: None,
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        }
    }

    /// 设置交易类型
    /// 
    /// # 参数
    /// - `tx_type`: 交易类型（Legacy 或 Eip1559）
    pub fn tx_type(mut self, tx_type: TxType) -> Self
    {
        self.tx_type = tx_type;
        self
    }

    /// 设置链 ID
    /// 
    /// # 参数
    /// - `chain_id`: 链 ID
    pub fn chain_id(mut self, chain_id: u64) -> Self
    {
        self.chain_id = Some(chain_id);
        self
    }

    /// 设置 nonce
    /// 
    /// # 参数
    /// - `nonce`: 交易序号
    pub fn nonce(mut self, nonce: u64) -> Self
    {
        self.nonce = Some(nonce);
        self
    }

    /// 设置接收地址
    /// 
    /// # 参数
    /// - `to`: 接收地址
    pub fn to(mut self, to: &str) -> Self
    {
        self.to = Some(utils::format_address(to));
        self
    }

    /// 设置转账金额（Wei）
    /// 
    /// # 参数
    /// - `value`: 转账金额
    pub fn value(mut self, value: U256) -> Self
    {
        self.value = value;
        self
    }

    /// 设置交易数据
    /// 
    /// # 参数
    /// - `data`: 交易数据（字节数组）
    pub fn data(mut self, data: Vec<u8>) -> Self
    {
        self.data = data;
        self
    }

    /// 设置 Gas 限制
    /// 
    /// # 参数
    /// - `gas_limit`: Gas 上限
    pub fn gas_limit(mut self, gas_limit: u64) -> Self
    {
        self.gas_limit = Some(gas_limit);
        self
    }

    /// 设置 Gas 价格（Legacy 交易）
    /// 
    /// # 参数
    /// - `gas_price`: Gas 价格（Wei）
    pub fn gas_price(mut self, gas_price: U256) -> Self
    {
        self.gas_price = Some(gas_price);
        self
    }

    /// 设置最大费用（EIP-1559）
    /// 
    /// # 参数
    /// - `max_fee`: 最大费用（Wei）
    pub fn max_fee_per_gas(mut self, max_fee: U256) -> Self
    {
        self.max_fee_per_gas = Some(max_fee);
        self
    }

    /// 设置最大优先费用（EIP-1559）
    /// 
    /// # 参数
    /// - `max_priority_fee`: 最大优先费用（Wei）
    pub fn max_priority_fee_per_gas(mut self, max_priority_fee: U256) -> Self
    {
        self.max_priority_fee_per_gas = Some(max_priority_fee);
        self
    }

    // ============ u128 接口（推荐使用）============

    /// 设置转账金额（使用 u128 Wei）
    /// 
    /// # 参数
    /// - `value`: 交易金额（u128 Wei）
    /// 
    /// # 返回
    /// - Self
    pub fn value_u128(mut self, value: u128) -> Self
    {
        // 将 u128 转换为 U256
        let mut bytes = [0u8; 32];
        bytes[16..32].copy_from_slice(&value.to_be_bytes());
        self.value = U256::from_be_slice(&bytes);
        self
    }

    /// 设置 Gas 价格（使用 u128 Wei，Legacy 交易）
    /// 
    /// # 参数
    /// - `value`: Gas 价格（u128 Wei）
    /// 
    /// # 返回
    /// - Self
    pub fn gas_price_u128(mut self, value: u128) -> Self
    {
        // 将 u128 转换为 U256
        let mut bytes = [0u8; 32];
        bytes[16..32].copy_from_slice(&value.to_be_bytes());
        self.gas_price = Some(U256::from_be_slice(&bytes));
        self
    }

    /// 设置最大费用（使用 u128 Wei，EIP-1559）
    /// 
    /// # 参数
    /// - `value`: 最大费用（u128 Wei）
    /// 
    /// # 返回
    /// - Self
    pub fn max_fee_per_gas_u128(mut self, value: u128) -> Self
    {
        // 将 u128 转换为 U256
        let mut bytes = [0u8; 32];
        bytes[16..32].copy_from_slice(&value.to_be_bytes());
        self.max_fee_per_gas = Some(U256::from_be_slice(&bytes));
        self
    }

    /// 设置最大优先费用（使用 u128 Wei，EIP-1559）
    /// 
    /// # 参数
    /// - `value`: 最大优先费用（u128 Wei）
    /// 
    /// # 返回
    /// - Self
    pub fn max_priority_fee_per_gas_u128(mut self, value: u128) -> Self
    {
        // 将 u128 转换为 U256
        let mut bytes = [0u8; 32];
        bytes[16..32].copy_from_slice(&value.to_be_bytes());
        self.max_priority_fee_per_gas = Some(U256::from_be_slice(&bytes));
        self
    }

    /// 构建交易
    /// 
    /// # 返回
    /// - Transaction 结构
    /// 
    /// # 错误
    /// - 如果缺少必要字段
    pub fn build(self) -> Result<Transaction, String>
    {
        let chain_id = self.chain_id.ok_or("chain_id is required")?;
        let nonce = self.nonce.ok_or("nonce is required")?;
        let gas_limit = self.gas_limit.ok_or("gas_limit is required")?;

        match self.tx_type
        {
            TxType::Legacy =>
            {
                let gas_price = self.gas_price.ok_or("gas_price is required for Legacy tx")?;
                Ok(Transaction {
                    tx_type: TxType::Legacy,
                    chain_id,
                    nonce,
                    to: self.to,
                    value: self.value,
                    data: self.data,
                    gas_limit,
                    gas_price: Some(gas_price),
                    max_fee_per_gas: None,
                    max_priority_fee_per_gas: None,
                })
            }
            TxType::Eip1559 =>
            {
                let max_fee = self.max_fee_per_gas.ok_or("max_fee_per_gas is required for EIP-1559 tx")?;
                let max_priority_fee = self.max_priority_fee_per_gas.ok_or("max_priority_fee_per_gas is required for EIP-1559 tx")?;
                Ok(Transaction {
                    tx_type: TxType::Eip1559,
                    chain_id,
                    nonce,
                    to: self.to,
                    value: self.value,
                    data: self.data,
                    gas_limit,
                    gas_price: None,
                    max_fee_per_gas: Some(max_fee),
                    max_priority_fee_per_gas: Some(max_priority_fee),
                })
            }
        }
    }
}

/// RLP 编码辅助函数
mod rlp
{
    use k256::elliptic_curve::bigint::U256;
    use k256::elliptic_curve::bigint::Encoding;

    /// 编码单个字节序列
    pub fn encode_bytes(data: &[u8]) -> Vec<u8>
    {
        if data.is_empty()
        {
            vec![0x80]
        }
        else if data.len() == 1 && data[0] < 0x80
        {
            data.to_vec()
        }
        else if data.len() <= 55
        {
            let mut result = vec![0x80 + data.len() as u8];
            result.extend_from_slice(data);
            result
        }
        else
        {
            let len_bytes = to_minimal_bytes(data.len() as u64);
            let mut result = vec![0xb7 + len_bytes.len() as u8];
            result.extend_from_slice(&len_bytes);
            result.extend_from_slice(data);
            result
        }
    }

    /// 编码 U256
    pub fn encode_u256(value: U256) -> Vec<u8>
    {
        if value == U256::ZERO
        {
            return vec![0x80];
        }
        let bytes = value.to_be_bytes();
        let trimmed: Vec<u8> = bytes.iter().skip_while(|&&b| b == 0).copied().collect();
        encode_bytes(&trimmed)
    }

    /// 编码 u64
    pub fn encode_u64(value: u64) -> Vec<u8>
    {
        if value == 0
        {
            return vec![0x80];
        }
        let bytes = value.to_be_bytes();
        let trimmed: Vec<u8> = bytes.iter().skip_while(|&&b| b == 0).copied().collect();
        encode_bytes(&trimmed)
    }

    /// 编码列表
    pub fn encode_list(items: &[Vec<u8>]) -> Vec<u8>
    {
        let mut content = Vec::new();
        for item in items
        {
            content.extend_from_slice(item);
        }

        if content.len() <= 55
        {
            let mut result = vec![0xc0 + content.len() as u8];
            result.extend_from_slice(&content);
            result
        }
        else
        {
            let len_bytes = to_minimal_bytes(content.len() as u64);
            let mut result = vec![0xf7 + len_bytes.len() as u8];
            result.extend_from_slice(&len_bytes);
            result.extend_from_slice(&content);
            result
        }
    }

    /// 将数字转换为最小字节表示
    fn to_minimal_bytes(value: u64) -> Vec<u8>
    {
        let bytes = value.to_be_bytes();
        bytes.iter().skip_while(|&&b| b == 0).copied().collect()
    }
}

/// 对交易进行 RLP 编码（用于签名）
/// 
/// # 参数
/// - `tx`: 交易结构
/// 
/// # 返回
/// - RLP 编码后的字节数组
fn encode_transaction_for_signing(tx: &Transaction) -> Vec<u8>
{
    match tx.tx_type
    {
        TxType::Legacy => encode_legacy_for_signing(tx),
        TxType::Eip1559 => encode_eip1559_for_signing(tx),
    }
}

/// Legacy 交易 RLP 编码（用于签名）
fn encode_legacy_for_signing(tx: &Transaction) -> Vec<u8>
{
    let to_bytes = match &tx.to
    {
        Some(addr) => utils::hex_to_bytes(addr).unwrap_or_default(),
        None => Vec::new(),
    };

    let items = vec![
        rlp::encode_u64(tx.nonce),
        rlp::encode_u256(tx.gas_price.unwrap()),
        rlp::encode_u64(tx.gas_limit),
        rlp::encode_bytes(&to_bytes),
        rlp::encode_u256(tx.value),
        rlp::encode_bytes(&tx.data),
        rlp::encode_u64(tx.chain_id),
        rlp::encode_u64(0),
        rlp::encode_u64(0),
    ];

    rlp::encode_list(&items)
}

/// EIP-1559 交易 RLP 编码（用于签名）
fn encode_eip1559_for_signing(tx: &Transaction) -> Vec<u8>
{
    let to_bytes = match &tx.to
    {
        Some(addr) => utils::hex_to_bytes(addr).unwrap_or_default(),
        None => Vec::new(),
    };

    let items = vec![
        rlp::encode_u64(tx.chain_id),
        rlp::encode_u64(tx.nonce),
        rlp::encode_u256(tx.max_priority_fee_per_gas.unwrap()),
        rlp::encode_u256(tx.max_fee_per_gas.unwrap()),
        rlp::encode_u64(tx.gas_limit),
        rlp::encode_bytes(&to_bytes),
        rlp::encode_u256(tx.value),
        rlp::encode_bytes(&tx.data),
        rlp::encode_bytes(&[]), // access list (empty)
    ];

    // EIP-1559 交易需要在前面加上 0x02 类型字节
    let mut result = vec![0x02];
    result.extend_from_slice(&rlp::encode_list(&items));
    result
}

/// 签名交易
/// 
/// # 参数
/// - `tx`: 交易结构
/// - `private_key`: 私钥（U256 格式）
/// 
/// # 返回
/// - SignedTransaction 结构
/// 
/// # 示例
/// ```
/// let signed = sign_transaction(&tx, &my_private_key)?;
/// let raw_tx = serialize_signed_transaction(&signed);
/// ```
pub fn sign_transaction(tx: &Transaction, private_key: &U256) -> Result<SignedTransaction, String>
{
    let encoded = encode_transaction_for_signing(tx);

    // 将 U256 转换为字节数组，然后创建 SecretKey
    let sk_bytes = private_key.to_be_bytes();
    let secret_key = SecretKey::from_be_bytes(&sk_bytes)
        .map_err(|e| format!("Invalid private key: {}", e))?;
    let signing_key = SigningKey::from(&secret_key);

    // 使用 Signer<recoverable::Signature> 签名原始编码数据
    // k256 内部会自动对 encoded 做 Keccak256 哈希，然后签名，并返回带 recovery id 的签名
    let recoverable_sig: recoverable::Signature = signing_key.sign(&encoded);

    let sig_bytes = recoverable_sig.as_ref();
    let r = U256::from_be_slice(&sig_bytes[0..32]);
    let s = U256::from_be_slice(&sig_bytes[32..64]);
    let recovery_id = recoverable_sig.recovery_id();

    let v = match tx.tx_type
    {
        TxType::Legacy =>
        {
            // EIP-155: v = chain_id * 2 + 35 + recovery_id
            tx.chain_id * 2 + 35 + u8::from(recovery_id) as u64
        }
        TxType::Eip1559 =>
        {
            // EIP-1559: v = recovery_id (0 or 1)
            u8::from(recovery_id) as u64
        }
    };

    Ok(SignedTransaction {
        transaction: tx.clone(),
        v,
        r,
        s,
    })
}

/// 序列化已签名交易
/// 
/// # 参数
/// - `signed_tx`: 已签名交易
/// 
/// # 返回
/// - 十六进制字符串（带 0x 前缀）
pub fn serialize_signed_transaction(signed_tx: &SignedTransaction) -> String
{
    let bytes = encode_signed_transaction(signed_tx);
    utils::add_0x_prefix(&utils::bytes_to_hex(&bytes))
}

/// 编码已签名交易
fn encode_signed_transaction(signed_tx: &SignedTransaction) -> Vec<u8>
{
    match signed_tx.transaction.tx_type
    {
        TxType::Legacy => encode_signed_legacy(signed_tx),
        TxType::Eip1559 => encode_signed_eip1559(signed_tx),
    }
}

/// 编码已签名的 Legacy 交易
fn encode_signed_legacy(signed_tx: &SignedTransaction) -> Vec<u8>
{
    let tx = &signed_tx.transaction;
    let to_bytes = match &tx.to
    {
        Some(addr) => utils::hex_to_bytes(addr).unwrap_or_default(),
        None => Vec::new(),
    };

    let items = vec![
        rlp::encode_u64(tx.nonce),
        rlp::encode_u256(tx.gas_price.unwrap()),
        rlp::encode_u64(tx.gas_limit),
        rlp::encode_bytes(&to_bytes),
        rlp::encode_u256(tx.value),
        rlp::encode_bytes(&tx.data),
        rlp::encode_u64(signed_tx.v),
        rlp::encode_u256(signed_tx.r),
        rlp::encode_u256(signed_tx.s),
    ];

    rlp::encode_list(&items)
}

/// 编码已签名的 EIP-1559 交易
fn encode_signed_eip1559(signed_tx: &SignedTransaction) -> Vec<u8>
{
    let tx = &signed_tx.transaction;
    let to_bytes = match &tx.to
    {
        Some(addr) => utils::hex_to_bytes(addr).unwrap_or_default(),
        None => Vec::new(),
    };

    let items = vec![
        rlp::encode_u64(tx.chain_id),
        rlp::encode_u64(tx.nonce),
        rlp::encode_u256(tx.max_priority_fee_per_gas.unwrap()),
        rlp::encode_u256(tx.max_fee_per_gas.unwrap()),
        rlp::encode_u64(tx.gas_limit),
        rlp::encode_bytes(&to_bytes),
        rlp::encode_u256(tx.value),
        rlp::encode_bytes(&tx.data),
        rlp::encode_bytes(&[]), // access list
        rlp::encode_u64(signed_tx.v),
        rlp::encode_u256(signed_tx.r),
        rlp::encode_u256(signed_tx.s),
    ];

    // EIP-1559 交易需要 0x02 前缀
    let mut result = vec![0x02];
    result.extend_from_slice(&rlp::encode_list(&items));
    result
}

/// 估算简单转账的 Gas 限制
/// 
/// # 返回
/// - 21000（标准转账 Gas）
fn simple_transfer_gas() -> u64
{
    21000
}

// ============ 跨 crate 公开接口（只使用 Rust 标准类型）============

/// 使用私钥（十六进制字符串）签名交易
/// 
/// # 参数
/// - `tx`: 交易结构
/// - `private_key_hex`: 私钥的十六进制字符串（可带或不带 0x 前缀）
/// 
/// # 返回
/// - SignedTransaction 结构
/// 
/// # 示例
/// ```
/// let signed = sign_transaction_with_hex(&tx, "0x1234...abcd")?;
/// let raw_tx = serialize_signed_transaction(&signed);
/// ```
pub fn sign_transaction_with_hex(tx: &Transaction, private_key_hex: &str) -> Result<SignedTransaction, String>
{
    let private_key = conversion::hex_to_k256_u256(private_key_hex)?;
    sign_transaction(tx, &private_key)
}

/// 使用字符串值构建简单转账交易
/// 
/// # 参数
/// - `to`: 接收地址
/// - `value_hex`: 转账金额（Wei，十六进制字符串）
/// - `chain_id`: 链 ID
/// - `nonce`: nonce
/// - `max_fee_per_gas_hex`: 最大费用（十六进制字符串）
/// - `max_priority_fee_per_gas_hex`: 最大优先费用（十六进制字符串）
/// 
/// # 返回
/// - Transaction 结构
pub fn build_transfer_tx(
    to: &str,
    value_hex: &str,
    chain_id: u64,
    nonce: u64,
    max_fee_per_gas_hex: &str,
    max_priority_fee_per_gas_hex: &str,
) -> Result<Transaction, String>
{
    let value = conversion::hex_to_k256_u256(value_hex)?;
    let max_fee = conversion::hex_to_k256_u256(max_fee_per_gas_hex)?;
    let max_priority_fee = conversion::hex_to_k256_u256(max_priority_fee_per_gas_hex)?;
    
    TransactionBuilder::new()
        .tx_type(TxType::Eip1559)
        .to(to)
        .value(value)
        .chain_id(chain_id)
        .nonce(nonce)
        .gas_limit(simple_transfer_gas())
        .max_fee_per_gas(max_fee)
        .max_priority_fee_per_gas(max_priority_fee)
        .build()
}
