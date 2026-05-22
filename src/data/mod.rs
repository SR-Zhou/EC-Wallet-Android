use crate::blockchain;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

pub use crate::blockchain::read::TxRecord;

#[derive(Debug, Clone, PartialEq)]
pub struct ChainInfo {
    pub name: &'static str,
    pub short_name: &'static str,
    pub chain_id: u64,
    pub rpc_url: &'static str,
    pub native_symbol: &'static str,
    pub native_icon: &'static str,
    pub chain_icon: &'static str,
    pub usdc_address: Option<&'static str>,
    pub usdt_address: Option<&'static str>,
    pub explorer_url: &'static str,
    pub blockscout_api_url: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TokenType {
    Native,
    USDC,
    USDT,
}

impl TokenType {
    pub fn symbol(&self, chain: &ChainInfo) -> String {
        match self {
            TokenType::Native => chain.native_symbol.to_string(),
            TokenType::USDC => "USDC".to_string(),
            TokenType::USDT => "USDT".to_string(),
        }
    }

    pub fn contract_address(&self, chain: &ChainInfo) -> Option<String> {
        match self {
            TokenType::Native => None,
            TokenType::USDC => chain.usdc_address.map(|s| s.to_string()),
            TokenType::USDT => chain.usdt_address.map(|s| s.to_string()),
        }
    }

    pub fn decimals(&self, chain: &ChainInfo) -> u8 {
        match self {
            TokenType::Native => 18,
            TokenType::USDC | TokenType::USDT => {
                if chain.chain_id == 56 {
                    18
                } else {
                    6
                }
            }
        }
    }
}

pub fn all_chains() -> Vec<ChainInfo> {
    vec![
        ChainInfo {
            name: "Ethereum",
            short_name: "Ethereum",
            chain_id: 1,
            rpc_url: "https://ethereum-rpc.publicnode.com",
            native_symbol: "ETH",
            native_icon: "🟣",
            chain_icon: "🟣",
            usdc_address: Some("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            usdt_address: Some("0xdAC17F958D2ee523a2206206994597C13D831ec7"),
            explorer_url: "https://etherscan.io",
            blockscout_api_url: Some("https://eth.blockscout.com/api/v2"),
        },
        ChainInfo {
            name: "Arbitrum One",
            short_name: "Arbitrum",
            chain_id: 42161,
            rpc_url: "https://arb1.arbitrum.io/rpc",
            native_symbol: "ETH",
            native_icon: "🟣",
            chain_icon: "🔵",
            usdc_address: Some("0xaf88d065e77c8cC2239327C5EDb3A432268e5831"),
            usdt_address: Some("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9"),
            explorer_url: "https://arbiscan.io",
            blockscout_api_url: Some("https://arbitrum.blockscout.com/api/v2"),
        },
        ChainInfo {
            name: "Base",
            short_name: "Base",
            chain_id: 8453,
            rpc_url: "https://mainnet.base.org",
            native_symbol: "ETH",
            native_icon: "🟣",
            chain_icon: "🟢",
            usdc_address: Some("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
            usdt_address: None,
            explorer_url: "https://basescan.org",
            blockscout_api_url: Some("https://base.blockscout.com/api/v2"),
        },
        ChainInfo {
            name: "Optimism",
            short_name: "Optimism",
            chain_id: 10,
            rpc_url: "https://mainnet.optimism.io",
            native_symbol: "ETH",
            native_icon: "🟣",
            chain_icon: "🔴",
            usdc_address: Some("0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85"),
            usdt_address: Some("0x94b008aA00579c1307B0EF2c499aD98a8ce58e58"),
            explorer_url: "https://optimistic.etherscan.io",
            blockscout_api_url: Some("https://optimism.blockscout.com/api/v2"),
        },
        ChainInfo {
            name: "Polygon",
            short_name: "Polygon",
            chain_id: 137,
            rpc_url: "https://polygon-rpc.com",
            native_symbol: "POL",
            native_icon: "🟠",
            chain_icon: "🟠",
            usdc_address: Some("0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"),
            usdt_address: Some("0xc2132D05D31c914a87C6611C10748AEb04B58e8F"),
            explorer_url: "https://polygonscan.com",
            blockscout_api_url: Some("https://polygon.blockscout.com/api/v2"),
        },
        ChainInfo {
            name: "BNB Smart Chain",
            short_name: "BSC",
            chain_id: 56,
            rpc_url: "https://bsc-dataseed.binance.org",
            native_symbol: "BNB",
            native_icon: "🟡",
            chain_icon: "🟡",
            usdc_address: Some("0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d"),
            usdt_address: Some("0x55d398326f99059fF775485246999027B3197955"),
            explorer_url: "https://bscscan.com",
            blockscout_api_url: None,
        },
        ChainInfo {
            name: "ZKsync",
            short_name: "ZKsync",
            chain_id: 324,
            rpc_url: "https://mainnet.era.zksync.io",
            native_symbol: "ETH",
            native_icon: "🟣",
            chain_icon: "🟦",
            usdc_address: Some("0x1d17CBcF0D6D143135aE902365D2E5e2A16538D4"),
            usdt_address: None,
            explorer_url: "https://explorer.zksync.io",
            blockscout_api_url: Some("https://zksync.blockscout.com/api/v2"),
        },
        ChainInfo {
            name: "Linea",
            short_name: "Linea",
            chain_id: 59144,
            rpc_url: "https://rpc.linea.build",
            native_symbol: "ETH",
            native_icon: "🟣",
            chain_icon: "🟤",
            usdc_address: Some("0x176211869cA2b568f2A7D4EE941E073a821EE1ff"),
            usdt_address: None,
            explorer_url: "https://lineascan.build",
            blockscout_api_url: None,
        },
        ChainInfo {
            name: "Scroll",
            short_name: "Scroll",
            chain_id: 534352,
            rpc_url: "https://rpc.scroll.io",
            native_symbol: "ETH",
            native_icon: "🟣",
            chain_icon: "📜",
            usdc_address: Some("0x06eFdBFf2a14a7c8E15944D1F4A48F9F95F663A4"),
            usdt_address: Some("0xf55BEC9cafDbE8730f096Aa55dad6D22d44099Df"),
            explorer_url: "https://scrollscan.com",
            blockscout_api_url: Some("https://scroll.blockscout.com/api/v2"),
        },
    ]
}

pub fn default_chain() -> ChainInfo {
    all_chains().into_iter().next().unwrap()
}

pub static BALANCE_CACHE: GlobalSignal<HashMap<String, Result<String, String>>> =
    GlobalSignal::new(HashMap::new);
pub static TX_CACHE: GlobalSignal<HashMap<String, Result<Vec<TxRecord>, String>>> =
    GlobalSignal::new(HashMap::new);
pub static TX_SERIAL: GlobalSignal<u32> = GlobalSignal::new(|| 0);

static ACTIVE_WALLET_ID: GlobalSignal<Option<String>> = GlobalSignal::new(|| None);
static REQUEST_QUEUE: GlobalSignal<VecDeque<RequestJob>> = GlobalSignal::new(VecDeque::new);
static REQUEST_KEYS: GlobalSignal<HashSet<RequestKey>> = GlobalSignal::new(HashSet::new);
static WORKER_STARTED: GlobalSignal<bool> = GlobalSignal::new(|| false);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RequestPriority {
    Low,
    Normal,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum RequestKind {
    Balance(TokenType),
    Transactions,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RequestKey {
    wallet_id: String,
    chain_id: u64,
    kind: RequestKind,
}

#[derive(Debug, Clone)]
struct RequestJob {
    key: RequestKey,
    wallet_id: String,
    address: String,
    chain: ChainInfo,
    priority: RequestPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WalletCache {
    balances: HashMap<String, Result<String, String>>,
    transactions: HashMap<String, Result<Vec<TxRecord>, String>>,
}

pub fn start_request_worker() {
    if *WORKER_STARTED.read() {
        return;
    }
    *WORKER_STARTED.write() = true;

    spawn(async move {
        loop {
            if let Some(job) = take_next_job() {
                execute_job(job).await;
            } else {
                futures_timer::Delay::new(std::time::Duration::from_millis(100)).await;
            }
        }
    });
}

pub fn activate_wallet(wallet_id: Option<String>) {
    if *ACTIVE_WALLET_ID.read() == wallet_id {
        return;
    }

    *ACTIVE_WALLET_ID.write() = wallet_id.clone();
    *BALANCE_CACHE.write() = HashMap::new();
    *TX_CACHE.write() = HashMap::new();

    if let Some(wallet_id) = wallet_id {
        if let Some(cache) = load_wallet_cache(&wallet_id) {
            *BALANCE_CACHE.write() = cache.balances;
            *TX_CACHE.write() = cache.transactions;
        }
    }

    *TX_SERIAL.write() += 1;
}

pub fn enqueue_balance(
    wallet_id: &str,
    address: &str,
    chain: ChainInfo,
    token: TokenType,
    priority: RequestPriority,
) {
    enqueue(RequestJob {
        key: RequestKey {
            wallet_id: wallet_id.to_string(),
            chain_id: chain.chain_id,
            kind: RequestKind::Balance(token),
        },
        wallet_id: wallet_id.to_string(),
        address: address.to_string(),
        chain,
        priority,
    });
}

pub fn enqueue_transactions(
    wallet_id: &str,
    address: &str,
    chain: ChainInfo,
    priority: RequestPriority,
) {
    enqueue(RequestJob {
        key: RequestKey {
            wallet_id: wallet_id.to_string(),
            chain_id: chain.chain_id,
            kind: RequestKind::Transactions,
        },
        wallet_id: wallet_id.to_string(),
        address: address.to_string(),
        chain,
        priority,
    });
}

pub fn balance_key(chain_id: u64, token: &TokenType) -> String {
    format!("{}_{:?}", chain_id, token)
}

pub fn tx_cache_key(chain_id: u64, token: &TokenType) -> String {
    balance_key(chain_id, token)
}

pub fn clear_balance(wallet_id: &str, chain_id: u64, token: &TokenType) {
    BALANCE_CACHE.write().remove(&balance_key(chain_id, token));
    save_wallet_cache(wallet_id);
}

pub fn clear_chain_balances(wallet_id: &str, chain_id: u64) {
    {
        let mut cache = BALANCE_CACHE.write();
        cache.remove(&balance_key(chain_id, &TokenType::Native));
        cache.remove(&balance_key(chain_id, &TokenType::USDC));
        cache.remove(&balance_key(chain_id, &TokenType::USDT));
    }
    save_wallet_cache(wallet_id);
}

pub fn clear_chain_transactions(wallet_id: &str, chain_id: u64) {
    {
        let mut cache = TX_CACHE.write();
        cache.remove(&tx_cache_key(chain_id, &TokenType::Native));
        cache.remove(&tx_cache_key(chain_id, &TokenType::USDC));
        cache.remove(&tx_cache_key(chain_id, &TokenType::USDT));
    }
    save_wallet_cache(wallet_id);
    *TX_SERIAL.write() += 1;
}

pub fn clear_transaction_group(wallet_id: &str, chain_id: u64, token: &TokenType) {
    TX_CACHE.write().remove(&tx_cache_key(chain_id, token));
    save_wallet_cache(wallet_id);
    *TX_SERIAL.write() += 1;
}

pub fn remove_wallet_cache(wallet_id: &str) {
    if wallet_is_active(wallet_id) {
        activate_wallet(None);
    }

    REQUEST_QUEUE
        .write()
        .retain(|job| job.wallet_id != wallet_id);
    REQUEST_KEYS
        .write()
        .retain(|key| key.wallet_id != wallet_id);

    let _ = std::fs::remove_file(get_cache_path(wallet_id));
}

pub fn cache_balance(
    wallet_id: &str,
    chain_id: u64,
    token: &TokenType,
    result: Result<String, String>,
) {
    if !wallet_is_active(wallet_id) {
        return;
    }

    BALANCE_CACHE
        .write()
        .insert(balance_key(chain_id, token), result);
    save_wallet_cache(wallet_id);
}

pub fn app_files_dir() -> Option<std::path::PathBuf> {
    let cmdline = std::fs::read("/proc/self/cmdline").ok()?;
    let package_name = String::from_utf8_lossy(&cmdline)
        .trim_matches(char::from(0))
        .to_string();
    if package_name.is_empty() {
        return None;
    }

    let app_dir = std::path::PathBuf::from(format!("/data/data/{}", package_name));
    let files_dir = app_dir.join("files");
    let _ = std::fs::create_dir_all(&files_dir);
    Some(files_dir)
}

fn enqueue(job: RequestJob) {
    if REQUEST_KEYS.read().contains(&job.key) {
        let mut queue = REQUEST_QUEUE.write();
        if let Some(queued) = queue.iter_mut().find(|queued| queued.key == job.key) {
            if job.priority > queued.priority {
                queued.priority = job.priority;
            }
        }
        return;
    }

    REQUEST_KEYS.write().insert(job.key.clone());
    REQUEST_QUEUE.write().push_back(job);
}

fn take_next_job() -> Option<RequestJob> {
    let mut queue = REQUEST_QUEUE.write();
    let index = queue
        .iter()
        .enumerate()
        .max_by_key(|(_, job)| job.priority)
        .map(|(index, _)| index)?;
    queue.remove(index)
}

async fn execute_job(job: RequestJob) {
    match job.key.kind.clone() {
        RequestKind::Balance(token) => {
            let result = if job.chain.chain_id == 137 {
                Err("Polygon 暂不支持余额查询".to_string())
            } else {
                let contract_address = token.contract_address(&job.chain);
                blockchain::read::fetch_balance(
                    &job.address,
                    job.chain.rpc_url,
                    contract_address.as_deref(),
                    token.decimals(&job.chain),
                )
                .await
            };
            cache_balance(&job.wallet_id, job.chain.chain_id, &token, result);
        }
        RequestKind::Transactions => {
            let query = blockchain::read::TransactionQuery {
                chain_name: job.chain.name.to_string(),
                blockscout_api_url: job.chain.blockscout_api_url.map(|url| url.to_string()),
                usdc_address: job.chain.usdc_address.map(|addr| addr.to_string()),
                usdt_address: job.chain.usdt_address.map(|addr| addr.to_string()),
            };
            let result = blockchain::read::fetch_transactions(&job.address, &query, job.chain.chain_id).await;
            cache_transactions(&job.wallet_id, job.chain.chain_id, result);
        }
    }

    REQUEST_KEYS.write().remove(&job.key);
    futures_timer::Delay::new(std::time::Duration::from_secs(1)).await;
}

fn cache_transactions(
    wallet_id: &str,
    chain_id: u64,
    result: Result<(Vec<TxRecord>, Vec<TxRecord>, Vec<TxRecord>), String>,
) {
    if !wallet_is_active(wallet_id) {
        return;
    }

    {
        let mut cache = TX_CACHE.write();
        match result {
            Ok((native_txs, usdc_txs, usdt_txs)) => {
                cache.insert(tx_cache_key(chain_id, &TokenType::Native), Ok(native_txs));
                cache.insert(tx_cache_key(chain_id, &TokenType::USDC), Ok(usdc_txs));
                cache.insert(tx_cache_key(chain_id, &TokenType::USDT), Ok(usdt_txs));
            }
            Err(error) => {
                cache.insert(
                    tx_cache_key(chain_id, &TokenType::Native),
                    Err(error.clone()),
                );
                cache.insert(tx_cache_key(chain_id, &TokenType::USDC), Err(error.clone()));
                cache.insert(tx_cache_key(chain_id, &TokenType::USDT), Err(error));
            }
        }
    }

    *TX_SERIAL.write() += 1;
    save_wallet_cache(wallet_id);
}

fn wallet_is_active(wallet_id: &str) -> bool {
    ACTIVE_WALLET_ID.read().as_deref() == Some(wallet_id)
}

fn get_cache_path(wallet_id: &str) -> std::path::PathBuf {
    let cache_dir = app_files_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("cache");
    let _ = std::fs::create_dir_all(&cache_dir);
    cache_dir.join(format!("{}.json", wallet_id))
}

fn load_wallet_cache(wallet_id: &str) -> Option<WalletCache> {
    let path = get_cache_path(wallet_id);
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn save_wallet_cache(wallet_id: &str) {
    if !wallet_is_active(wallet_id) {
        return;
    }

    let cache = WalletCache {
        balances: BALANCE_CACHE.read().clone(),
        transactions: TX_CACHE.read().clone(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&cache) {
        let _ = std::fs::write(get_cache_path(wallet_id), json);
    }
}
