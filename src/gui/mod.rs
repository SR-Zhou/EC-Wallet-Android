use crate::blockchain;
use crate::crypto;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

// ============ 常量 ============
const WALLETS_DIR: &str = "wallets";
const WALLET_LIST_FILE: &str = "wallet_list.json";
const API_KEY_FILE: &str = "etherscan_api_key";

// ============ 应用状态 ============

/// 应用页面
#[derive(Debug, Clone, PartialEq)]
enum Page
{
    /// 钱包列表页面（选择/管理钱包）
    WalletList,
    /// 登录/解锁页面（指定钱包 id）
    Login(String),
    /// 导入钱包页面
    Import,
    /// 主钱包页面
    Wallet,
    /// 发送页面
    Send,
    /// 接收页面
    Receive,
    /// 交换页面
    Swap,
}

// ============ 钱包列表数据结构 ============

/// 单个钱包信息（存储在 wallet_list.json 中）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct WalletInfo
{
    /// 唯一 ID（用作 keystore 文件名）
    pub id: String,
    /// 用户给钱包起的名称
    pub name: String,
    /// 钱包地址（0x...），导入时计算并存储，显示时无需解密
    pub address: String,
    /// 公钥 X 坐标（十六进制），导入时计算并存储
    #[serde(default)]
    pub public_key_x: String,
    /// 公钥 Y 坐标（十六进制），导入时计算并存储
    #[serde(default)]
    pub public_key_y: String,
    /// 创建时间（Unix 时间戳秒）
    pub created_at: u64,
}

/// 钱包列表（wallet_list.json 的根结构）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct WalletList
{
    pub wallets: Vec<WalletInfo>,
    pub default_wallet_id: Option<String>,
}

/// 简化的交易记录（用于 UI 显示）
#[derive(Debug, Clone, PartialEq)]
struct TxRecord
{
    pub hash: String,
    pub from: String,
    pub to: String,
    pub value_eth: String,
    pub value_wei: String,
    pub timestamp: String,
    pub block_number: String,
    pub gas_used: String,
    pub is_error: bool,
    /// true = 发出, false = 收到
    pub is_outgoing: bool,
}

/// 链信息（GUI 用）
#[derive(Debug, Clone, PartialEq)]
struct ChainInfo
{
    /// 显示名称
    pub name: &'static str,
    /// 短名称（选项卡显示用）
    pub short_name: &'static str,
    /// 链 ID
    pub chain_id: u64,
    /// RPC URL
    pub rpc_url: &'static str,
    /// 原生代币符号（如 ETH、MATIC、BNB）
    pub native_symbol: &'static str,
    /// 原生代币图标
    pub native_icon: &'static str,
    /// 链图标
    pub chain_icon: &'static str,
    /// USDC 合约地址（None 表示该链不支持）
    pub usdc_address: Option<&'static str>,
    /// USDT 合约地址（None 表示该链不支持）
    pub usdt_address: Option<&'static str>,
    /// Etherscan V2 API 基础 URL
    pub explorer_api_url: &'static str,
    /// 区块浏览器 URL
    pub explorer_url: &'static str,
}

/// 获取所有支持的链
fn all_chains() -> Vec<ChainInfo>
{
    vec![
        ChainInfo {
            name: "Ethereum",
            short_name: "Ethereum",
            chain_id: 1,
            rpc_url: "https://ethereum-rpc.publicnode.com",
            native_symbol: "ETH",
            native_icon: "🔷",
            chain_icon: "Ξ",
            usdc_address: Some("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            usdt_address: Some("0xdAC17F958D2ee523a2206206994597C13D831ec7"),
            explorer_api_url: "https://api.etherscan.io",
            explorer_url: "https://etherscan.io",
        },
        ChainInfo {
            name: "Arbitrum One",
            short_name: "Arbitrum",
            chain_id: 42161,
            rpc_url: "https://arb1.arbitrum.io/rpc",
            native_symbol: "ETH",
            native_icon: "🔷",
            chain_icon: "🔵",
            usdc_address: Some("0xaf88d065e77c8cC2239327C5EDb3A432268e5831"),
            usdt_address: Some("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9"),
            explorer_api_url: "https://api.etherscan.io",
            explorer_url: "https://arbiscan.io",
        },
        ChainInfo {
            name: "Base",
            short_name: "Base",
            chain_id: 8453,
            rpc_url: "https://mainnet.base.org",
            native_symbol: "ETH",
            native_icon: "🔷",
            chain_icon: "🔵",
            usdc_address: Some("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
            usdt_address: None,
            explorer_api_url: "https://api.etherscan.io",
            explorer_url: "https://basescan.org",
        },
        ChainInfo {
            name: "Optimism",
            short_name: "Optimism",
            chain_id: 10,
            rpc_url: "https://mainnet.optimism.io",
            native_symbol: "ETH",
            native_icon: "🔷",
            chain_icon: "🔴",
            usdc_address: Some("0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85"),
            usdt_address: Some("0x94b008aA00579c1307B0EF2c499aD98a8ce58e58"),
            explorer_api_url: "https://api.etherscan.io",
            explorer_url: "https://optimistic.etherscan.io",
        },
        ChainInfo {
            name: "Polygon",
            short_name: "Polygon",
            chain_id: 137,
            rpc_url: "https://polygon-rpc.com",
            native_symbol: "POL",
            native_icon: "🟣",
            chain_icon: "🟣",
            usdc_address: Some("0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"),
            usdt_address: Some("0xc2132D05D31c914a87C6611C10748AEb04B58e8F"),
            explorer_api_url: "https://api.etherscan.io",
            explorer_url: "https://polygonscan.com",
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
            explorer_api_url: "https://api.etherscan.io",
            explorer_url: "https://bscscan.com",
        },
        ChainInfo {
            name: "Avalanche C-Chain",
            short_name: "Avalanche",
            chain_id: 43114,
            rpc_url: "https://api.avax.network/ext/bc/C/rpc",
            native_symbol: "AVAX",
            native_icon: "🔺",
            chain_icon: "🔺",
            usdc_address: Some("0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E"),
            usdt_address: Some("0x9702230A8Ea53601f5cD2dc00fDBc13d4dF4A8c7"),
            explorer_api_url: "https://api.etherscan.io",
            explorer_url: "https://snowtrace.io",
        },
        ChainInfo {
            name: "zkSync Era",
            short_name: "zkSync",
            chain_id: 324,
            rpc_url: "https://mainnet.era.zksync.io",
            native_symbol: "ETH",
            native_icon: "🔷",
            chain_icon: "🔷",
            usdc_address: Some("0x1d17CBcF0D6D143135aE902365D2E5e2A16538D4"),
            usdt_address: None,
            explorer_api_url: "https://api.etherscan.io",
            explorer_url: "https://explorer.zksync.io",
        },
        ChainInfo {
            name: "Linea",
            short_name: "Linea",
            chain_id: 59144,
            rpc_url: "https://rpc.linea.build",
            native_symbol: "ETH",
            native_icon: "🔷",
            chain_icon: "➖",
            usdc_address: Some("0x176211869cA2b568f2A7D4EE941E073a821EE1ff"),
            usdt_address: None,
            explorer_api_url: "https://api.etherscan.io",
            explorer_url: "https://lineascan.build",
        },
        ChainInfo {
            name: "Scroll",
            short_name: "Scroll",
            chain_id: 534352,
            rpc_url: "https://rpc.scroll.io",
            native_symbol: "ETH",
            native_icon: "🔷",
            chain_icon: "📜",
            usdc_address: Some("0x06eFdBFf2a14a7c8E15944D1F4A48F9F95F663A4"),
            usdt_address: Some("0xf55BEC9cafDbE8730f096Aa55dad6D22d44099Df"),
            explorer_api_url: "https://api.etherscan.io",
            explorer_url: "https://scrollscan.com",
        },
    ]
}

/// 获取默认链（Ethereum）
fn default_chain() -> ChainInfo
{
    all_chains().into_iter().next().unwrap()
}

/// 支持的代币类型
#[derive(Debug, Clone, PartialEq)]
enum TokenType
{
    Native,
    USDC,
    USDT,
}

impl TokenType
{
    /// 获取代币符号（根据当前链）
    fn symbol(&self, chain: &ChainInfo) -> String
    {
        match self
        {
            TokenType::Native => chain.native_symbol.to_string(),
            TokenType::USDC => "USDC".to_string(),
            TokenType::USDT => "USDT".to_string(),
        }
    }

    /// ERC20 合约地址（根据当前链），Native 返回 None
    fn contract_address(&self, chain: &ChainInfo) -> Option<String>
    {
        match self
        {
            TokenType::Native => None,
            TokenType::USDC => chain.usdc_address.map(|s| s.to_string()),
            TokenType::USDT => chain.usdt_address.map(|s| s.to_string()),
        }
    }

    /// 代币精度（根据链返回不同精度）
    fn decimals(&self, chain: &ChainInfo) -> u8
    {
        match self
        {
            TokenType::Native => 18,
            TokenType::USDC | TokenType::USDT => {
                // BNB Smart Chain (chain_id: 56) 上的 USDC 和 USDT 使用 18 位精度
                if chain.chain_id == 56 {
                    18
                } else {
                    6
                }
            }
        }
    }}

// ============ 入口 ============

pub fn launch()
{
    dioxus::launch(app);
}

// ============ 主组件 ============

fn app() -> Element
{
    let page = use_signal(|| {
        let list = load_wallet_list();
        if list.wallets.is_empty()
        {
            Page::Import
        }
        else if let Some(ref default_id) = list.default_wallet_id
        {
            Page::Login(default_id.clone())
        }
        else
        {
            Page::WalletList
        }
    });
    rsx! {
        style { {CSS} }
        div { class: "app-container",
            match page() {
                Page::WalletList => rsx! { WalletListPage { page } },
                Page::Login(wallet_id) => rsx! { LoginPage { page, wallet_id } },
                Page::Import => rsx! { ImportPage { page } },
                Page::Wallet => rsx! { WalletPage { page } },
                Page::Send => rsx! { SendPage { page } },
                Page::Receive => rsx! { ReceivePage { page } },
                Page::Swap => rsx! { SwapPage { page } },
            }
        }
    }
}

// ============ 登录页面 ============

#[component]
fn LoginPage(page: Signal<Page>, wallet_id: String) -> Element
{
    let mut password = use_signal(|| String::new());
    let error_msg = use_signal(|| String::new());
    let loading = use_signal(|| false);

    // 获取钱包信息用于显示
    let wallet_list = load_wallet_list();
    let wallet_info = wallet_list.wallets.iter().find(|w| w.id == wallet_id).cloned();
    let wallet_name = wallet_info.as_ref().map(|w| w.name.clone()).unwrap_or_else(|| "未知钱包".to_string());
    let wallet_address = wallet_info.as_ref().map(|w| shorten_address(&w.address)).unwrap_or_default();
    let wid = wallet_id.clone();

    let do_unlock_fn = {
        let wid = wid.clone();
        move |password: &Signal<String>, mut error_msg: Signal<String>, mut loading: Signal<bool>, mut page: Signal<Page>| {
            let pw = password().trim().to_string();
            if pw.is_empty()
            {
                error_msg.set("请输入密码".into());
                return;
            }
            loading.set(true);
            error_msg.set(String::new());

            let keystore_path = get_wallet_keystore_path(&wid);
            match crypto::keystore::load(pw, &keystore_path, false)
            {
                Ok(sk_hex) =>
                {
                    match blockchain::account::private_key_hex_to_address(&sk_hex)
                    {
                        Ok(addr) =>
                        {
                            // 更新钱包列表中的地址和公钥
                            let mut list = load_wallet_list();
                            if let Some(w) = list.wallets.iter_mut().find(|w| w.id == wid)
                            {
                                let mut changed = false;
                                if w.address.starts_with('（') || w.address.is_empty()
                                {
                                    w.address = addr;
                                    changed = true;
                                }
                                if w.public_key_x.is_empty()
                                {
                                    if let Ok((px, py)) = blockchain::account::private_key_hex_to_public_key_xy(&sk_hex)
                                    {
                                        w.public_key_x = px;
                                        w.public_key_y = py;
                                        changed = true;
                                    }
                                }
                                if changed
                                {
                                    save_wallet_list(&list);
                                }
                            }

                            // 记录当前活跃钱包 id
                            *CURRENT_WALLET_ID.write() = Some(wid.clone());
                            UNLOCKED_SK.write().replace(sk_hex);
                            page.set(Page::Wallet);
                        }
                        Err(e) =>
                        {
                            error_msg.set(format!("私钥无效: {}", e));
                        }
                    }
                }
                Err(e) =>
                {
                    error_msg.set(format!("解锁失败: {}", e));
                }
            }
            loading.set(false);
        }
    };

    let go_import = move |_: Event<MouseData>| {
        page.set(Page::Import);
    };

    let go_wallet_list = move |_: Event<MouseData>| {
        page.set(Page::WalletList);
    };

    let has_multiple_wallets = wallet_list.wallets.len() > 1;

    rsx! {
        div { class: "page login-page",
            div { class: "card",
                h1 { class: "title", "🔐 EC Wallet" }
                p { class: "subtitle", "解锁钱包: {wallet_name}" }
                if !wallet_address.is_empty() {
                    p { class: "subtitle", style: "margin-top: -16px; font-family: monospace; font-size: 12px; color: #666;", "{wallet_address}" }
                }

                div { class: "form-group",
                    label { "密码" }
                    input {
                        r#type: "password",
                        placeholder: "请输入密码...",
                        value: "{password}",
                        oninput: move |e| password.set(e.value()),
                        onkeypress: {
                            let do_unlock_fn = do_unlock_fn.clone();
                            move |e: Event<KeyboardData>| {
                                if e.key() == Key::Enter {
                                    do_unlock_fn(&password, error_msg, loading, page);
                                }
                            }
                        },
                    }
                }

                if !error_msg().is_empty() {
                    p { class: "error", "{error_msg}" }
                }

                button {
                    class: "btn btn-primary",
                    disabled: loading(),
                    onclick: move |_: Event<MouseData>| do_unlock_fn(&password, error_msg, loading, page),
                    if loading() { "解锁中..." } else { "解锁" }
                }

                div { class: "divider" }

                if has_multiple_wallets {
                    button {
                        class: "btn btn-secondary",
                        style: "margin-bottom: 8px;",
                        onclick: go_wallet_list,
                        "📋 切换钱包"
                    }
                }

                button {
                    class: "btn btn-secondary",
                    onclick: go_import,
                    "导入新钱包"
                }
            }
        }
    }
}

// ============ 导入钱包页面 ============

#[component]
fn ImportPage(page: Signal<Page>) -> Element
{
    let mut private_key = use_signal(|| String::new());
    let mut password = use_signal(|| String::new());
    let mut password_confirm = use_signal(|| String::new());
    let mut wallet_name_input = use_signal(|| String::new());
    let mut error_msg = use_signal(|| String::new());
    let mut loading = use_signal(|| false);

    let on_import = move |_| {
        let sk = private_key().trim().to_string();
        let pw = password().trim().to_string();
        let pw2 = password_confirm().trim().to_string();
        let name = wallet_name_input().trim().to_string();
        if sk.is_empty()
        {
            error_msg.set("请输入私钥".into());
            return;
        }
        if name.is_empty()
        {
            error_msg.set("请输入钱包名称".into());
            return;
        }
        if pw.is_empty()
        {
            error_msg.set("请输入密码".into());
            return;
        }
        if pw != pw2
        {
            error_msg.set("两次密码不一致".into());
            return;
        }

        let sk_clean = if sk.starts_with("0x") || sk.starts_with("0X")
        {
            sk[2..].to_string()
        }
        else
        {
            sk.clone()
        };

        if !blockchain::conversion::is_valid_private_key(&sk_clean)
        {
            error_msg.set("私钥格式无效".into());
            return;
        }

        // 计算地址
        let address = match blockchain::account::private_key_hex_to_address(&sk_clean)
        {
            Ok(addr) => addr,
            Err(e) =>
            {
                error_msg.set(format!("私钥无效: {}", e));
                return;
            }
        };

        loading.set(true);
        error_msg.set(String::new());

        // 生成钱包 ID
        let wallet_id = match generate_wallet_id(&sk_clean)
        {
            Ok(id) => id,
            Err(e) =>
            {
                error_msg.set(format!("生成钱包 ID 失败: {}", e));
                loading.set(false);
                return;
            }
        };
        let wallet_name = name;

        // 存储 keystore 文件
        let keystore_path = get_wallet_keystore_path(&wallet_id);
        match crypto::keystore::store(pw, sk_clean.clone(), &keystore_path, false)
        {
            Ok(()) =>
            {
                // 计算公钥
                let (pk_x, pk_y) = blockchain::account::private_key_hex_to_public_key_xy(&sk_clean)
                    .unwrap_or_default();

                // 添加到钱包列表
                let info = WalletInfo {
                    id: wallet_id.clone(),
                    name: wallet_name,
                    address,
                    public_key_x: pk_x,
                    public_key_y: pk_y,
                    created_at: current_unix_timestamp(),
                };
                let mut list = load_wallet_list();
                // 如果是第一个钱包，设为默认
                let is_first = list.wallets.is_empty();
                list.wallets.push(info);
                if is_first
                {
                    list.default_wallet_id = Some(wallet_id.clone());
                }
                save_wallet_list(&list);

                *CURRENT_WALLET_ID.write() = Some(wallet_id);
                UNLOCKED_SK.write().replace(sk_clean);
                page.set(Page::Wallet);
            }
            Err(e) =>
            {
                error_msg.set(format!("导入失败: {}", e));
            }
        }
        loading.set(false);
    };

    let go_login = move |_| {
        let list = load_wallet_list();
        if !list.wallets.is_empty()
        {
            if let Some(ref default_id) = list.default_wallet_id
            {
                page.set(Page::Login(default_id.clone()));
            }
            else
            {
                page.set(Page::WalletList);
            }
        }
    };

    let go_wallet_list = move |_| {
        page.set(Page::WalletList);
    };

    let has_wallets = !load_wallet_list().wallets.is_empty();

    rsx! {
        div { class: "page import-page",
            div { class: "card",
                h1 { class: "title", "📥 导入钱包" }
                p { class: "subtitle", "通过私钥导入你的以太坊钱包" }

                div { class: "form-group",
                    label { "钱包名称" }
                    input {
                        r#type: "text",
                        placeholder: "例如: 主钱包",
                        value: "{wallet_name_input}",
                        oninput: move |e| wallet_name_input.set(e.value()),
                    }
                }

                div { class: "form-group",
                    label { "私钥（十六进制）" }
                    input {
                        r#type: "password",
                        placeholder: "输入私钥 (hex)...",
                        value: "{private_key}",
                        oninput: move |e| private_key.set(e.value()),
                    }
                }

                div { class: "form-group",
                    label { "设置密码" }
                    input {
                        r#type: "password",
                        placeholder: "设置密码...",
                        value: "{password}",
                        oninput: move |e| password.set(e.value()),
                    }
                }

                div { class: "form-group",
                    label { "确认密码" }
                    input {
                        r#type: "password",
                        placeholder: "再次输入密码...",
                        value: "{password_confirm}",
                        oninput: move |e| password_confirm.set(e.value()),
                    }
                }

                if !error_msg().is_empty() {
                    p { class: "error", "{error_msg}" }
                }

                button {
                    class: "btn btn-primary",
                    disabled: loading(),
                    onclick: on_import,
                    if loading() { "导入中..." } else { "导入钱包" }
                }

                if has_wallets {
                    div { class: "divider" }
                    button {
                        class: "btn btn-secondary",
                        style: "margin-bottom: 8px;",
                        onclick: go_wallet_list,
                        "📋 钱包列表"
                    }
                    button {
                        class: "btn btn-secondary",
                        onclick: go_login,
                        "返回登录"
                    }
                }
            }
        }
    }
}

// ============ 钱包列表页面 ============

#[component]
fn WalletListPage(page: Signal<Page>) -> Element
{
    let mut wallet_list = use_signal(|| load_wallet_list());
    let mut confirm_delete: Signal<Option<String>> = use_signal(|| None);
    let mut show_wallet_detail: Signal<Option<WalletInfo>> = use_signal(|| None);
    let mut renaming_id: Signal<Option<String>> = use_signal(|| None);
    let mut rename_input = use_signal(|| String::new());

    let mut on_select = move |id: String| {
        page.set(Page::Login(id));
    };

    let mut on_set_default = move |id: String| {
        let mut list = wallet_list();
        list.default_wallet_id = Some(id);
        save_wallet_list(&list);
        wallet_list.set(list);
    };

    let mut on_delete = move |id: String| {
        let mut list = wallet_list();
        // 删除 keystore 文件
        let path = get_wallet_keystore_path(&id);
        let _ = std::fs::remove_file(path);
        // 从列表移除
        list.wallets.retain(|w| w.id != id);
        // 如果删除的是默认钱包，重置默认
        if list.default_wallet_id.as_ref() == Some(&id)
        {
            list.default_wallet_id = list.wallets.first().map(|w| w.id.clone());
        }
        save_wallet_list(&list);
        wallet_list.set(list);
        confirm_delete.set(None);
    };

    let go_import = move |_| {
        page.set(Page::Import);
    };

    rsx! {
        div { class: "page wallet-list-page",
            div { class: "topbar",
                span { class: "topbar-title", "🔷 EC Wallet" }
                span {}
            }

            div { class: "card",
                h2 { class: "section-title", "📋 钱包列表" }
                p { class: "subtitle", style: "text-align: left; margin-bottom: 16px;", "选择要解锁的钱包，或导入新钱包" }

                div { class: "wallet-list",
                    {
                        let list = wallet_list();
                        let default_id = list.default_wallet_id.clone().unwrap_or_default();
                        rsx! {
                            for wallet in list.wallets.iter() {
                                {
                                    let w = wallet.clone();
                                    let w_id = w.id.clone();
                                    let w_id_default = w.id.clone();
                                    let w_id_delete = w.id.clone();
                                    let w_id_rename = w.id.clone();
                                    let w_detail = w.clone();
                                    let is_default = w.id == default_id;
                                    let short_addr = shorten_address(&w.address);
                                    let confirming = confirm_delete() == Some(w.id.clone());
                                    rsx! {
                                                div { class: if is_default { "wallet-list-item default" } else { "wallet-list-item" },
                                            div {
                                                class: "wallet-list-main",
                                                onclick: move |_| on_select(w_id.clone()),
                                                div { class: "wallet-list-info",
                                                    div { class: "wallet-list-name",
                                                        span { "{w.name}" }
                                                        if is_default {
                                                            span { class: "wallet-default-badge", "默认" }
                                                        }
                                                    }
                                                    span { class: "wallet-list-addr", "{short_addr}" }
                                                }
                                                span { class: "wallet-list-arrow", "→" }
                                            }
                                            {
                                                let is_renaming = renaming_id() == Some(w.id.clone());
                                                let w_name_for_rename = w.name.clone();
                                                let w_id_rename_confirm = w_id_rename.clone();
                                                rsx! {                                                    if is_renaming {
                                                        div { class: "wallet-rename-row",
                                                            input {
                                                                class: "wallet-rename-input",
                                                                r#type: "text",
                                                                value: "{rename_input}",
                                                                oninput: move |e| rename_input.set(e.value()),
                                                                placeholder: "新名称...",
                                                            }
                                                            button {
                                                                class: "btn btn-small btn-primary",
                                                                onclick: move |_| {
                                                                    let new_name = rename_input().trim().to_string();
                                                                    if !new_name.is_empty() {
                                                                        let mut list = wallet_list();
                                                                        if let Some(w) = list.wallets.iter_mut().find(|w| w.id == w_id_rename_confirm) {
                                                                            w.name = new_name;
                                                                        }
                                                                        save_wallet_list(&list);
                                                                        wallet_list.set(list);
                                                                    }
                                                                    renaming_id.set(None);
                                                                },
                                                                "确认"
                                                            }
                                                            button {
                                                                class: "btn btn-small btn-secondary",
                                                                onclick: move |_| renaming_id.set(None),
                                                                "取消"
                                                            }
                                                        }
                                                    }                                                    div {
                                                        class: "wallet-list-actions",
                                                        if !is_default {
                                                            button {
                                                                class: "btn btn-small btn-secondary",
                                                                onclick: move |_| on_set_default(w_id_default.clone()),
                                                                "设为默认"
                                                            }
                                                        }
                                                        if !is_renaming {
                                                            button {
                                                                class: "btn btn-small btn-secondary",
                                                                onclick: move |_| {
                                                                    rename_input.set(w_name_for_rename.clone());
                                                                    renaming_id.set(Some(w_id_rename.clone()));
                                                                },
                                                                "改名"
                                                            }
                                                        }
                                                        if !confirming {
                                                            button {
                                                                class: "btn btn-small btn-danger",
                                                                onclick: move |_| confirm_delete.set(Some(w_id_delete.clone())),
                                                                "删除"
                                                            }
                                                        }
                                                        if confirming {
                                                            {
                                                                let del_id = w.id.clone();
                                                                rsx! {
                                                                    span { class: "delete-confirm-text", "确认删除？" }
                                                                    button {
                                                                        class: "btn btn-small btn-danger",
                                                                        onclick: move |_| on_delete(del_id.clone()),
                                                                        "确认"
                                                                    }
                                                                    button {
                                                                        class: "btn btn-small btn-secondary",
                                                                        onclick: move |_| confirm_delete.set(None),
                                                                        "取消"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        button {
                                                            class: "btn btn-small btn-secondary",
                                                            onclick: move |_| show_wallet_detail.set(Some(w_detail.clone())),
                                                            "详细信息"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if wallet_list().wallets.is_empty() {
                    p { class: "empty-text", "暂无钱包，请导入" }
                }

                div { class: "divider" }
                button {
                    class: "btn btn-primary",
                    onclick: go_import,
                    "➕ 导入新钱包"
                }
            }

            // ---- 钱包详情模态窗口 ----
            if let Some(detail_wallet) = show_wallet_detail() {
                WalletDetailModal {
                    wallet: detail_wallet,
                    on_close: move || show_wallet_detail.set(None),
                }
            }
        }
    }
}

// ============ 钱包主页面 ============

#[component]
fn WalletPage(page: Signal<Page>) -> Element
{
    let address = use_signal(|| {
        let sk = UNLOCKED_SK.read();
        match sk.as_ref()
        {
            Some(sk_hex) => blockchain::account::private_key_hex_to_address(sk_hex)
                .unwrap_or_else(|_| "地址计算错误".to_string()),
            None => "未解锁".to_string(),
        }
    });
    let mut selected_token = use_signal(|| TokenType::Native);
    let mut show_chain_modal = use_signal(|| false);
    let mut show_wallet_dropdown = use_signal(|| false);
    let mut show_wallet_detail: Signal<Option<WalletInfo>> = use_signal(|| None);
    let mut renaming_id: Signal<Option<String>> = use_signal(|| None);
    let mut rename_input = use_signal(|| String::new());
    let mut confirm_delete: Signal<Option<String>> = use_signal(|| None);
    let mut wallet_list_version = use_signal(|| 0u32); // 用于触发钱包列表刷新
    let mut balance = use_signal(|| "加载中...".to_string());
    let mut transactions: Signal<Vec<TxRecord>> = use_signal(Vec::new);
    let mut tx_loading = use_signal(|| false);
    let mut tx_error = use_signal(|| String::new());
    let mut api_key_input = use_signal(|| ETHERSCAN_KEY.read().clone());
    let mut api_key_saved = use_signal(|| false);

    let addr = address();
    let addr_for_tx = addr.clone();

    // 余额随 selected_token / selected_chain 变化而重新获取
    use_effect(move || {
        let addr = address().clone();
        let token = selected_token().clone();
        let chain = SELECTED_CHAIN.read().clone();
        balance.set("加载中...".to_string());
        spawn(async move {
            match fetch_token_balance(&addr, &token, &chain).await
            {
                Ok(bal) => balance.set(bal),
                Err(e) => balance.set(format!("错误: {}", e)),
            }
        });
    });

    use_effect(move || {
        let addr = addr_for_tx.clone();
        let chain = SELECTED_CHAIN.read().clone();
        tx_loading.set(true);
        tx_error.set(String::new());
        spawn(async move {
            match fetch_transactions(&addr, &chain).await
            {
                Ok(txs) =>
                {
                    transactions.set(txs);
                }
                Err(e) =>
                {
                    tx_error.set(format!("获取交易历史失败: {}", e));
                }
            }
            tx_loading.set(false);
        });
    });

    let on_lock = move |_| {
        // 安全清除内存中的私钥
        if let Some(mut sk) = UNLOCKED_SK.write().take() {
            sk.zeroize();
        }
        *CURRENT_WALLET_ID.write() = None;
        let list = load_wallet_list();
        if let Some(ref default_id) = list.default_wallet_id
        {
            page.set(Page::Login(default_id.clone()));
        }
        else if !list.wallets.is_empty()
        {
            page.set(Page::WalletList);
        }
        else
        {
            page.set(Page::Import);
        }
    };
    let addr_display = address();
    let short_addr = shorten_address(&addr_display);
    let chain = SELECTED_CHAIN.read().clone();
    let native_symbol = chain.native_symbol.to_string();
    let native_icon = chain.native_icon;
    let chain_display_name = chain.short_name.to_string();
    let chain_icon = chain.chain_icon;
    let has_usdt = chain.usdt_address.is_some();

    rsx! {
        div { class: "page wallet-page",
            div { class: "topbar",
                span { class: "topbar-title", "🔷 EC Wallet" }
                button { class: "btn btn-small btn-danger", onclick: on_lock, "🔒 锁定" }
            }

            // ---- 钱包选择器按钮 ----
            {
                let _ = wallet_list_version(); // 读取版本号以触发响应式更新
                let wallet_list = load_wallet_list();
                let current_wid = CURRENT_WALLET_ID.read().clone().unwrap_or_default();
                let current_wallet_info = wallet_list.wallets.iter().find(|w| w.id == current_wid).cloned();
                let current_wallet_name = current_wallet_info.as_ref().map(|w| w.name.clone()).unwrap_or_else(|| "未知钱包".to_string());
                let default_wid = wallet_list.default_wallet_id.clone().unwrap_or_default();
                
                rsx! {
                    div { class: "wallet-selector-container",
                        button {
                            class: "wallet-selector-btn",
                            onclick: move |_| show_wallet_dropdown.set(!show_wallet_dropdown()),
                            span { class: "wallet-selector-icon", "👛" }
                            span { class: "wallet-selector-name", "{current_wallet_name}" }
                            span { class: "wallet-selector-arrow", if show_wallet_dropdown() { "▴" } else { "▾" } }
                        }

                        // 下拉面板
                        if show_wallet_dropdown() {
                            div { class: "wallet-dropdown",
                                for wallet in wallet_list.wallets.iter() {
                                    {
                                        let w = wallet.clone();
                                        let w_id = w.id.clone();
                                        let w_id_default = w.id.clone();
                                        let w_id_rename = w.id.clone();
                                        let w_id_rename_confirm = w.id.clone();
                                        let w_id_delete = w.id.clone();
                                        let w_name_for_rename = w.name.clone();
                                        let w_detail = w.clone();
                                        let is_current = w.id == current_wid;
                                        let is_default = w.id == default_wid;
                                        let short_addr = shorten_address(&w.address);
                                        let is_renaming = renaming_id() == Some(w.id.clone());
                                        let confirming = confirm_delete() == Some(w.id.clone());
                                        rsx! {
                                            div { class: if is_current { "wallet-dropdown-item current" } else { "wallet-dropdown-item" },
                                                div {
                                                    class: "wallet-dropdown-main",
                                                    onclick: move |_| {
                                                        if !is_current {
                                                            // 切换到该钱包，需要重新解锁
                                                            UNLOCKED_SK.write().take();
                                                            page.set(Page::Login(w_id.clone()));
                                                        }
                                                        show_wallet_dropdown.set(false);
                                                    },
                                                    div { class: "wallet-dropdown-info",
                                                        div { class: "wallet-dropdown-name",
                                                            span { "{w.name}" }
                                                            if is_current {
                                                                span { class: "wallet-current-badge", "当前" }
                                                            }
                                                            if is_default {
                                                                span { class: "wallet-default-badge", "默认" }
                                                            }
                                                        }
                                                        span { class: "wallet-dropdown-addr", "{short_addr}" }
                                                    }
                                                }
                                                if is_renaming {
                                                    div { class: "wallet-rename-row",
                                                        input {
                                                            class: "wallet-rename-input",
                                                            r#type: "text",
                                                            value: "{rename_input}",
                                                            oninput: move |e| rename_input.set(e.value()),
                                                            placeholder: "新名称...",
                                                        }
                                                        button {
                                                            class: "btn btn-small btn-primary",
                                                            onclick: move |_| {
                                                                let new_name = rename_input().trim().to_string();
                                                                if !new_name.is_empty() {
                                                                    let mut list = load_wallet_list();
                                                                    if let Some(w) = list.wallets.iter_mut().find(|w| w.id == w_id_rename_confirm) {
                                                                        w.name = new_name;
                                                                    }
                                                                    save_wallet_list(&list);
                                                                }
                                                                renaming_id.set(None);
                                                            },
                                                            "确认"
                                                        }
                                                        button {
                                                            class: "btn btn-small btn-secondary",
                                                            onclick: move |_| renaming_id.set(None),
                                                            "取消"
                                                        }
                                                    }
                                                }
                                                div { class: "wallet-dropdown-actions",
                                                    if !is_default {
                                                        button {
                                                            class: "btn btn-small btn-secondary",
                                                            onclick: move |_| {
                                                                let mut list = load_wallet_list();
                                                                list.default_wallet_id = Some(w_id_default.clone());
                                                                save_wallet_list(&list);
                                                                wallet_list_version.set(wallet_list_version() + 1);
                                                            },
                                                            "设为默认"
                                                        }
                                                    }
                                                    if !is_renaming {
                                                        button {
                                                            class: "btn btn-small btn-secondary",
                                                            onclick: move |_| {
                                                                rename_input.set(w_name_for_rename.clone());
                                                                renaming_id.set(Some(w_id_rename.clone()));
                                                            },
                                                            "改名"
                                                        }
                                                    }
                                                    if !confirming && !is_renaming {
                                                        button {
                                                            class: "btn btn-small btn-danger",
                                                            onclick: move |_| confirm_delete.set(Some(w_id_delete.clone())),
                                                            "删除"
                                                        }
                                                    }
                                                    if confirming {
                                                        {
                                                            let del_id = w.id.clone();
                                                            let curr_wid = current_wid.clone();
                                                            rsx! {
                                                                span { class: "delete-confirm-text", "确认删除？" }
                                                                button {
                                                                    class: "btn btn-small btn-danger",
                                                                    onclick: move |_| {
                                                                        // 删除钱包
                                                                        let mut list = load_wallet_list();
                                                                        list.wallets.retain(|w| w.id != del_id);
                                                                        // 如果删除的是默认钱包，自动选择第一个钱包为新默认
                                                                        if list.default_wallet_id.as_ref() == Some(&del_id) {
                                                                            list.default_wallet_id = list.wallets.first().map(|w| w.id.clone());
                                                                        }
                                                                        // 如果删除的是当前钱包，返回钱包列表页面
                                                                        if del_id == curr_wid {
                                                                            save_wallet_list(&list);
                                                                            UNLOCKED_SK.write().take();
                                                                            page.set(Page::WalletList);
                                                                        } else {
                                                                            save_wallet_list(&list);
                                                                        }
                                                                        confirm_delete.set(None);
                                                                        show_wallet_dropdown.set(false);
                                                                    },
                                                                    "确认"
                                                                }
                                                                button {
                                                                    class: "btn btn-small btn-secondary",
                                                                    onclick: move |_| confirm_delete.set(None),
                                                                    "取消"
                                                                }
                                                            }
                                                        }
                                                    }
                                                    button {
                                                        class: "btn btn-small btn-secondary",
                                                        onclick: move |_| {
                                                            show_wallet_detail.set(Some(w_detail.clone()));
                                                            show_wallet_dropdown.set(false);
                                                        },
                                                        "详细信息"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                div { class: "wallet-dropdown-footer",
                                    button {
                                        class: "btn btn-small btn-primary",
                                        onclick: move |_| {
                                            show_wallet_dropdown.set(false);
                                            page.set(Page::Import);
                                        },
                                        "➕ 导入新钱包"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ---- 链选择器按钮 ----
            button {
                class: "chain-selector-btn",
                onclick: move |_| show_chain_modal.set(true),
                span { class: "chain-selector-icon", "{chain_icon}" }
                span { class: "chain-selector-name", "{chain_display_name}" }
                span { class: "chain-selector-arrow", "▾" }
            }

            div { class: "token-tabs",
                button {
                    class: if selected_token() == TokenType::Native { "token-tab active" } else { "token-tab" },
                    onclick: move |_| selected_token.set(TokenType::Native),
                    span { class: "token-tab-icon", "{native_icon}" }
                    span { "{native_symbol}" }
                }
                button {
                    class: if selected_token() == TokenType::USDC { "token-tab active" } else { "token-tab" },
                    onclick: move |_| selected_token.set(TokenType::USDC),
                    span { class: "token-tab-icon", "🔵" }
                    span { "USDC" }
                }
                if has_usdt {
                    button {
                        class: if selected_token() == TokenType::USDT { "token-tab active" } else { "token-tab" },
                        onclick: move |_| selected_token.set(TokenType::USDT),
                        span { class: "token-tab-icon", "🟢" }
                        span { "USDT" }
                    }
                }
            }

            div { class: "card account-card",
                div { class: "account-address",
                    span { class: "label", "地址" }
                    span { class: "address", title: "{addr_display}", "{short_addr}" }
                }
                div { class: "account-balance",
                    span { class: "balance-value", "{balance}" }
                    span { class: "balance-unit", "{selected_token().symbol(&chain)}" }
                }
                div { class: "action-buttons",
                    button {
                        class: "btn btn-action",
                        onclick: move |_| page.set(Page::Send),
                        "📤 发送"
                    }
                    button {
                        class: "btn btn-action",
                        onclick: move |_| page.set(Page::Receive),
                        "📥 接收"
                    }
                    button { class: "btn btn-action", onclick: move |_| page.set(Page::Swap), "🔄 交换" }
                }
            }

            div { class: "card tx-card",
                h2 { class: "section-title", "📋 交易历史" }

                if tx_loading() {
                    p { class: "loading-text", "加载交易历史中..." }
                }

                if !tx_error().is_empty() {
                    p { class: "error", "{tx_error}" }
                }

                if transactions().is_empty() && !tx_loading() && tx_error().is_empty() {
                    p { class: "empty-text", "暂无交易记录" }
                }

                div { class: "tx-list",
                    for tx in transactions() {
                        TxItem { tx: tx.clone(), my_address: addr_display.clone() }
                    }
                }
            }
            if ETHERSCAN_KEY.read().is_empty() {
                div { class: "card",
                    h2 { class: "section-title", "⚙️ Etherscan API Key" }
                    p { class: "subtitle", style: "margin-bottom: 12px; text-align: left;",
                        "查看交易历史需要 Etherscan API Key（可在 "
                        a { href: "https://etherscan.io/myapikey", style: "color: #667eea;", "etherscan.io" }
                        " 免费注册获取）"
                    }
                    div { class: "form-group",
                        input {
                            r#type: "password",
                            placeholder: "输入 Etherscan API Key...",
                            value: "{api_key_input}",
                            oninput: move |e| {
                                api_key_input.set(e.value());
                                api_key_saved.set(false);
                            },
                        }
                    }
                    button {
                        class: "btn btn-primary",
                        style: "margin-bottom: 8px;",
                        onclick: move |_| {
                            let key = api_key_input().trim().to_string();
                            save_api_key(&key);
                            *ETHERSCAN_KEY.write() = key;
                            api_key_saved.set(true);
                        },
                        "💾 保存 API Key"
                    }
                    if api_key_saved() {
                        p { style: "color: #6bff6b; font-size: 13px; text-align: center;", "✅ 已保存！重新进入钱包页面后生效" }
                    }
                }
            }
            // ---- 链选择模态窗口 ----
            if show_chain_modal() {
                ChainSelectorModal {
                    on_close: move || show_chain_modal.set(false),
                    on_select: move |chain: ChainInfo| {
                        *SELECTED_CHAIN.write() = chain;
                        // 切链后重置到 Native 代币
                        selected_token.set(TokenType::Native);
                        show_chain_modal.set(false);
                    },
                }
            }

            // ---- 钱包详情模态窗口 ----
            if let Some(detail_wallet) = show_wallet_detail() {
                WalletDetailModal {
                    wallet: detail_wallet,
                    on_close: move || show_wallet_detail.set(None),
                }
            }
        }
    }
}

// ============ 发送页面 ============

#[component]
fn SendPage(page: Signal<Page>) -> Element
{
    let address = {
        let sk = UNLOCKED_SK.read();
        match sk.as_ref()
        {
            Some(sk_hex) => blockchain::account::private_key_hex_to_address(sk_hex)
                .unwrap_or_else(|_| "地址计算错误".to_string()),
            None => "未解锁".to_string(),
        }
    };

    let mut selected_token = use_signal(|| TokenType::Native);
    let mut recipient = use_signal(|| String::new());
    let mut amount_str = use_signal(|| String::new());
    let mut error_msg = use_signal(|| String::new());
    let mut success_msg = use_signal(|| String::new());
    let mut sending = use_signal(|| false);
    let mut gas_fee_percent = use_signal(|| 5u32); // 默认 5%
    let mut fetching_balance = use_signal(|| false);
    let mut max_amount_units: Signal<Option<u128>> = use_signal(|| None); // 存储精确的最小单位余额

    let my_addr = address.clone();
    let my_addr_for_max = my_addr.clone(); // 用于 on_set_max
    let my_addr_for_send = my_addr.clone(); // 用于 on_send

    // 点击"全部"按钮的处理函数
    let on_set_max = move |_| {
        let token = selected_token();
        let addr = my_addr_for_max.clone();
        let chain = SELECTED_CHAIN.read().clone();
        
        fetching_balance.set(true);
        error_msg.set(String::new());
        
        spawn(async move {
            // 获取余额（返回最小单位）
            let balance_units = match fetch_token_balance_units(&addr, &token, &chain).await {
                Ok(units) => units,
                Err(e) => {
                    error_msg.set(format!("获取余额失败: {}", e));
                    fetching_balance.set(false);
                    return;
                }
            };
            
            if balance_units == 0 {
                error_msg.set("余额为零".into());
                fetching_balance.set(false);
                return;
            }
            
            // 存储精确的余额（最小单位）
            max_amount_units.set(Some(balance_units));
            
            // 格式化为人类可读的字符串显示
            let decimals = token.decimals(&chain);
            let display_str = format_units_to_decimal(balance_units, decimals);
            amount_str.set(display_str);
            
            fetching_balance.set(false);
        });
    };

    let on_send = move |_| {
        let to_addr = recipient().trim().to_string();
        let amount_input = amount_str().trim().to_string();
        let token = selected_token();
        let from_addr = my_addr_for_send.clone();
        let chain = SELECTED_CHAIN.read().clone();
        let gas_percent = gas_fee_percent(); // 获取选择的 gas fee 百分比

        // 基本验证
        if to_addr.is_empty()
        {
            error_msg.set("请输入接收地址".into());
            return;
        }
        if !blockchain::utils::is_valid_address(&to_addr)
        {
            error_msg.set("接收地址格式无效".into());
            return;
        }
        if amount_input.is_empty()
        {
            error_msg.set("请输入金额".into());
            return;
        }

        let sk_hex = match UNLOCKED_SK.read().clone()
        {
            Some(sk) => sk,
            None =>
            {
                error_msg.set("钱包未解锁".into());
                return;
            }
        };

        let stored_max = max_amount_units(); // 获取存储的精确最大金额

        sending.set(true);
        error_msg.set(String::new());
        success_msg.set(String::new());

        spawn(async move {
            // 解析金额为最小单位（纯整数，不经过浮点数）
            // 如果有存储的最大金额且输入框的值与格式化后的最大金额相近，则使用精确值
            let amount_units = if let Some(max_units) = stored_max {
                // 使用存储的精确最大金额
                max_units
            } else {
                // 正常解析用户输入
                match token {
                    TokenType::Native => {
                        // 原生代币：18 位小数
                        match parse_token_amount_to_units(&amount_input, 18) {
                            Ok(v) => v,
                            Err(e) => {
                                error_msg.set(e);
                                sending.set(false);
                                return;
                            }
                        }
                    }
                    TokenType::USDC | TokenType::USDT => {
                        // ERC20 代币：根据链动态获取精度（BSC 为 18 位，其他链为 6 位）
                        let decimals = token.decimals(&chain);
                        match parse_token_amount_to_units(&amount_input, decimals) {
                            Ok(v) => v,
                            Err(e) => {
                                error_msg.set(e);
                                sending.set(false);
                                return;
                            }
                        }
                    }
                }
            };

            if amount_units == 0 {
                error_msg.set("金额必须大于零".into());
                sending.set(false);
                return;
            }

            let result = match token {
                TokenType::Native => {
                    // 发送原生代币
                    send_native_transaction_units(&from_addr, &to_addr, amount_units, &sk_hex, &chain, gas_percent).await
                }
                TokenType::USDC | TokenType::USDT => {
                    // 发送 ERC20 代币
                    let contract_addr = match token.contract_address(&chain) {
                        Some(addr) => addr,
                        None => {
                            error_msg.set(format!("该链不支持 {}", token.symbol(&chain)));
                            sending.set(false);
                            return;
                        }
                    };
                    send_erc20_transaction(&from_addr, &to_addr, amount_units, &contract_addr, &sk_hex, &chain, gas_percent).await
                }
            };

            match result {
                Ok(tx_hash) =>
                {
                    success_msg.set(format!("交易已发送！\n交易哈希: {}", tx_hash));
                    recipient.set(String::new());
                    amount_str.set(String::new());
                }
                Err(e) =>
                {
                    error_msg.set(format!("发送失败: {}", e));
                }
            }
            sending.set(false);
        });
    };

    let chain = SELECTED_CHAIN.read().clone();
    let native_symbol = chain.native_symbol.to_string();
    let native_icon = chain.native_icon;
    let has_usdt = chain.usdt_address.is_some();
    let token_symbol_title = selected_token().symbol(&chain);
    let token_symbol_label = selected_token().symbol(&chain);

    rsx! {
        div { class: "page send-page",
            div { class: "topbar",
                button {
                    class: "btn btn-small btn-secondary",
                    onclick: move |_| page.set(Page::Wallet),
                    "← 返回"
                }
                span { class: "topbar-title", "📤 发送" }
                span {}
            }

            div { class: "token-tabs",
                button {
                    class: if selected_token() == TokenType::Native { "token-tab active" } else { "token-tab" },
                    onclick: move |_| selected_token.set(TokenType::Native),
                    span { class: "token-tab-icon", "{native_icon}" }
                    span { "{native_symbol}" }
                }
                button {
                    class: if selected_token() == TokenType::USDC { "token-tab active" } else { "token-tab" },
                    onclick: move |_| selected_token.set(TokenType::USDC),
                    span { class: "token-tab-icon", "🔵" }
                    span { "USDC" }
                }
                if has_usdt {
                    button {
                        class: if selected_token() == TokenType::USDT { "token-tab active" } else { "token-tab" },
                        onclick: move |_| selected_token.set(TokenType::USDT),
                        span { class: "token-tab-icon", "🟢" }
                        span { "USDT" }
                    }
                }
            }

            div { class: "card",
                h2 { class: "section-title", "发送 {token_symbol_title}" }

                div { class: "form-group",
                    label { "接收地址" }
                    input {
                        r#type: "text",
                        placeholder: "0x...",
                        value: "{recipient}",
                        oninput: move |e| recipient.set(e.value()),
                    }
                }

                div { class: "form-group",
                    div { class: "form-label-row",
                        label { "金额 ({token_symbol_label})" }
                        button {
                            class: "btn-max",
                            disabled: fetching_balance(),
                            onclick: on_set_max,
                            if fetching_balance() { "加载中..." } else { "全部" }
                        }
                    }
                    input {
                        r#type: "text",
                        placeholder: "0.0",
                        value: "{amount_str}",
                        oninput: move |e| {
                            amount_str.set(e.value());
                            // 手动输入时清除存储的最大金额
                            max_amount_units.set(None);
                        },
                    }
                }

                div { class: "form-group",
                    label { "Gas Fee" }
                    div { class: "gas-fee-buttons",
                        button {
                            class: if gas_fee_percent() == 2 { "gas-fee-btn active" } else { "gas-fee-btn" },
                            onclick: move |_| gas_fee_percent.set(2),
                            "2%"
                        }
                        button {
                            class: if gas_fee_percent() == 5 { "gas-fee-btn active" } else { "gas-fee-btn" },
                            onclick: move |_| gas_fee_percent.set(5),
                            "5%"
                        }
                        button {
                            class: if gas_fee_percent() == 8 { "gas-fee-btn active" } else { "gas-fee-btn" },
                            onclick: move |_| gas_fee_percent.set(8),
                            "8%"
                        }
                    }
                    p { class: "gas-fee-hint", "基础 Gas Price 上浮相应百分比，提高交易确认速度" }
                }

                if !error_msg().is_empty() {
                    p { class: "error", "{error_msg}" }
                }

                if !success_msg().is_empty() {
                    div { class: "success-box",
                        p { "✅ 交易已发送！" }
                        p { class: "tx-hash-display", "{success_msg}" }
                    }
                }

                button {
                    class: "btn btn-primary",
                    disabled: sending(),
                    onclick: on_send,
                    if sending() { "发送中..." } else { "确认发送" }
                }

                div { class: "notice",
                    "⚠️ 请仔细核对接收地址和金额，交易一旦发送无法撤销。"
                }
            }
        }
    }
}

// ============ 接收页面 ============

#[component]
fn ReceivePage(page: Signal<Page>) -> Element
{
    let address = {
        let sk = UNLOCKED_SK.read();
        match sk.as_ref()
        {
            Some(sk_hex) => blockchain::account::private_key_hex_to_address(sk_hex)
                .unwrap_or_else(|_| "地址计算错误".to_string()),
            None => "未解锁".to_string(),
        }
    };

    let mut copied = use_signal(|| false);

    rsx! {
        div { class: "page receive-page",
            div { class: "topbar",
                button {
                    class: "btn btn-small btn-secondary",
                    onclick: move |_| page.set(Page::Wallet),
                    "← 返回"
                }
                span { class: "topbar-title", "📥 接收" }
                span {}
            }

            div { class: "card",
                h2 { class: "section-title", style: "text-align: center;", "你的钱包地址" }
                p { class: "subtitle", "将以下地址分享给发送方，即可接收 ETH 和 ERC20 代币。" }

                div { class: "address-display",
                    p { class: "full-address", "{address}" }
                }
                button {
                    class: "btn btn-primary",
                    style: "margin-top: 16px;",
                    r#type: "button",
                    onclick: move |_| {
                        copied.set(true);
                    },
                    // 使用内联 JavaScript 处理剪切板（WebView 支持）
                    "onmousedown": format!("navigator.clipboard && navigator.clipboard.writeText('{}');", address),
                    if copied() { "✅ 已复制" } else { "📋 复制地址" }
                }

                if copied() {
                    p { class: "copy-hint", "地址已复制到剪贴板" }
                }
            }
        }
    }
}

// ============ 交换页面 ============

/// 交换方向的代币类型（支持所有已知代币）
#[derive(Debug, Clone, PartialEq)]
enum SwapToken
{
    Native,
    USDC,
    USDT,
}

impl SwapToken
{
    fn symbol(&self, chain: &ChainInfo) -> String
    {
        match self
        {
            SwapToken::Native => chain.native_symbol.to_string(),
            SwapToken::USDC => "USDC".to_string(),
            SwapToken::USDT => "USDT".to_string(),
        }
    }

    fn decimals(&self, chain: &ChainInfo) -> u8
    {
        match self
        {
            SwapToken::Native => 18,
            SwapToken::USDC | SwapToken::USDT => {
                // BNB Smart Chain (chain_id: 56) 上的 USDC 和 USDT 使用 18 位精度
                if chain.chain_id == 56 {
                    18
                } else {
                    6
                }
            }
        }
    }

    /// 返回代币合约地址；原生代币返回 "native"
    fn address(&self, chain: &ChainInfo) -> Option<String>
    {
        match self
        {
            SwapToken::Native => Some("native".to_string()),
            SwapToken::USDC => chain.usdc_address.map(|s| s.to_string()),
            SwapToken::USDT => chain.usdt_address.map(|s| s.to_string()),
        }
    }

    /// 返回获取报价时使用的地址（原生代币用 WETH）
    fn quote_address(&self, chain: &ChainInfo) -> Option<String>
    {
        match self
        {
            SwapToken::Native => blockchain::swap::weth_address(chain.chain_id).map(|s| s.to_string()),
            SwapToken::USDC => chain.usdc_address.map(|s| s.to_string()),
            SwapToken::USDT => chain.usdt_address.map(|s| s.to_string()),
        }
    }

    /// 转换为 TokenType（用于查询余额）
    fn to_token_type(&self) -> TokenType
    {
        match self
        {
            SwapToken::Native => TokenType::Native,
            SwapToken::USDC => TokenType::USDC,
            SwapToken::USDT => TokenType::USDT,
        }
    }
}

#[component]
fn SwapPage(page: Signal<Page>) -> Element
{
    let chain = SELECTED_CHAIN.read().clone();
    let chain_supported = blockchain::swap::is_chain_supported(chain.chain_id);

    let address = {
        let sk = UNLOCKED_SK.read();
        match sk.as_ref()
        {
            Some(sk_hex) => blockchain::account::private_key_hex_to_address(sk_hex)
                .unwrap_or_else(|_| "地址计算错误".to_string()),
            None => "未解锁".to_string(),
        }
    };

    let mut token_from = use_signal(|| SwapToken::Native);
    let mut token_to = use_signal(|| SwapToken::USDC);
    let mut amount_str = use_signal(|| String::new());
    let mut slippage_str = use_signal(|| "0.5".to_string());
    let mut fee_tier = use_signal(|| 3000u32);
    let mut quote_result = use_signal(|| String::new());
    let mut quote_loading = use_signal(|| false);
    let mut error_msg = use_signal(|| String::new());
    let mut success_msg = use_signal(|| String::new());
    let mut swapping = use_signal(|| false);
    let mut quote_amount_out: Signal<Option<u128>> = use_signal(|| None);
    let mut fetching_swap_balance = use_signal(|| false);
    let mut max_swap_amount_units: Signal<Option<u128>> = use_signal(|| None); // 存储精确的最小单位余额

    let addr_for_swap = address.clone();
    let addr_for_max = address.clone();

    // 点击"全部"按钮的处理函数（交换页面）
    let on_set_max_swap = move |_| {
        let token = token_from();
        let token_type = token.to_token_type();
        let addr = addr_for_max.clone();
        let chain = SELECTED_CHAIN.read().clone();
        
        fetching_swap_balance.set(true);
        error_msg.set(String::new());
        
        spawn(async move {
            // 获取余额（返回最小单位）
            let balance_units = match fetch_token_balance_units(&addr, &token_type, &chain).await {
                Ok(units) => units,
                Err(e) => {
                    error_msg.set(format!("获取余额失败: {}", e));
                    fetching_swap_balance.set(false);
                    return;
                }
            };
            
            if balance_units == 0 {
                error_msg.set("余额为零".into());
                fetching_swap_balance.set(false);
                return;
            }
            
            // 存储精确的余额（最小单位）
            max_swap_amount_units.set(Some(balance_units));
            
            // 格式化为人类可读的字符串显示
            let decimals = token.decimals(&chain);
            let display_str = format_units_to_decimal(balance_units, decimals);
            amount_str.set(display_str);
            
            // 清除之前的报价
            quote_result.set(String::new());
            quote_amount_out.set(None);
            
            fetching_swap_balance.set(false);
        });
    };

    // 获取报价
    let on_get_quote = move |_| {
        let amount_input = amount_str().trim().to_string();
        let from = token_from();
        let to = token_to();
        let chain = SELECTED_CHAIN.read().clone();
        let fee = fee_tier();
        let stored_max = max_swap_amount_units(); // 获取存储的精确最大金额

        if from == to
        {
            error_msg.set("输入和输出代币不能相同".into());
            return;
        }

        if amount_input.is_empty()
        {
            error_msg.set("请输入金额".into());
            return;
        }

        // 根据代币类型解析金额（纯整数，不经过浮点数）
        // 如果有存储的最大金额，则使用精确值
        let amount_in: u128 = if let Some(max_units) = stored_max {
            // 使用存储的精确最大金额
            max_units
        } else {
            // 正常解析用户输入
            match from {
                SwapToken::Native => {
                    // 原生代币：18 位小数
                    match parse_token_amount_to_units(&amount_input, 18) {
                        Ok(v) => v,
                        Err(e) => {
                            error_msg.set(e);
                            return;
                        }
                    }
                }
                SwapToken::USDC | SwapToken::USDT => {
                    // ERC20 代币：根据链动态获取精度（BSC 为 18 位，其他链为 6 位）
                    let decimals = from.decimals(&chain);
                    match parse_token_amount_to_units(&amount_input, decimals) {
                        Ok(v) => v,
                        Err(e) => {
                            error_msg.set(e);
                            return;
                        }
                    }
                }
            }
        };

        if amount_in == 0 {
            error_msg.set("金额必须大于零".into());
            return;
        }

        let token_in_addr = match from.quote_address(&chain)
        {
            Some(addr) => addr,
            None =>
            {
                error_msg.set("该链不支持此输入代币".into());
                return;
            }
        };

        let token_out_addr = match to.quote_address(&chain)
        {
            Some(addr) => addr,
            None =>
            {
                error_msg.set("该链不支持此输出代币".into());
                return;
            }
        };

        let amount_in_hex = format!("0x{:x}", amount_in);

        error_msg.set(String::new());
        success_msg.set(String::new());
        quote_loading.set(true);
        quote_result.set(String::new());
        quote_amount_out.set(None);

        let to_decimals = to.decimals(&chain);
        let to_symbol = to.symbol(&chain);

        spawn(async move {
            let rpc = blockchain::rpc::RpcClient::new(chain.rpc_url);
            match blockchain::swap::get_quote(&rpc, &token_in_addr, &token_out_addr, &amount_in_hex, fee).await
            {
                Ok(quote) =>
                {
                    let formatted = blockchain::swap::format_token_amount(quote.amount_out, to_decimals);
                    quote_result.set(format!("≈ {} {}", formatted, to_symbol));
                    quote_amount_out.set(Some(quote.amount_out));
                }
                Err(e) =>
                {
                    error_msg.set(format!("获取报价失败: {}", e));
                }
            }
            quote_loading.set(false);
        });
    };

    // 执行交换
    let on_swap = move |_| {
        let amount_input = amount_str().trim().to_string();
        let from = token_from();
        let to = token_to();
        let chain = SELECTED_CHAIN.read().clone();
        let fee = fee_tier();
        let my_addr = addr_for_swap.clone();

        if from == to
        {
            error_msg.set("输入和输出代币不能相同".into());
            return;
        }

        if amount_input.is_empty()
        {
            error_msg.set("请输入金额".into());
            return;
        }

        // 根据代币类型解析金额（与 on_get_quote 保持一致，纯整数）
        let amount_in: u128 = match from {
            SwapToken::Native => {
                // 原生代币：18 位小数
                match parse_token_amount_to_units(&amount_input, 18) {
                    Ok(v) => v,
                    Err(e) => {
                        error_msg.set(e);
                        return;
                    }
                }
            }
            SwapToken::USDC | SwapToken::USDT => {
                // ERC20 代币：根据链动态获取精度（BSC 为 18 位，其他链为 6 位）
                let decimals = from.decimals(&chain);
                match parse_token_amount_to_units(&amount_input, decimals) {
                    Ok(v) => v,
                    Err(e) => {
                        error_msg.set(e);
                        return;
                    }
                }
            }
        };

        if amount_in == 0 {
            error_msg.set("金额必须大于零".into());
            return;
        }

        // 解析滑点（存储为整数，表示百分比的 10 倍，例如 5 表示 0.5%）
        let slippage_times_10: u32 = match slippage_str().trim().parse::<f32>()
        {
            Ok(v) if v > 0.0 && v < 50.0 => (v * 10.0) as u32,
            _ =>
            {
                error_msg.set("滑点必须在 0-50% 之间".into());
                return;
            }
        };

        let quoted_out = match quote_amount_out()
        {
            Some(v) => v,
            None =>
            {
                error_msg.set("请先获取报价".into());
                return;
            }
        };

        // 计算最小输出（考虑滑点）- 使用纯整数运算
        // amount_out_min = quoted_out * (1000 - slippage_times_10) / 1000
        let amount_out_min = quoted_out * (1000 - slippage_times_10 as u128) / 1000;

        let token_in_addr = match from.address(&chain)
        {
            Some(addr) => addr,
            None =>
            {
                error_msg.set("该链不支持此输入代币".into());
                return;
            }
        };

        let token_out_addr = match to.address(&chain)
        {
            Some(addr) => addr,
            None =>
            {
                error_msg.set("该链不支持此输出代币".into());
                return;
            }
        };

        let sk_hex = match UNLOCKED_SK.read().clone()
        {
            Some(sk) => sk,
            None =>
            {
                error_msg.set("钱包未解锁".into());
                return;
            }
        };

        swapping.set(true);
        error_msg.set(String::new());
        success_msg.set(String::new());

        let swap_params = blockchain::swap::SwapParams {
            token_in: token_in_addr,
            token_out: token_out_addr,
            amount_in,
            amount_out_min,
            fee,
            recipient: my_addr,
            chain_id: chain.chain_id,
        };

        spawn(async move {
            let rpc = blockchain::rpc::RpcClient::new(chain.rpc_url);
            match blockchain::swap::execute_swap(&rpc, &swap_params, &sk_hex).await
            {
                Ok(tx_hash) =>
                {
                    success_msg.set(format!("交换成功！交易哈希: {}", tx_hash));
                    amount_str.set(String::new());
                    quote_result.set(String::new());
                    quote_amount_out.set(None);
                }
                Err(e) =>
                {
                    error_msg.set(format!("交换失败: {}", e));
                }
            }
            swapping.set(false);
        });
    };

    let native_symbol = chain.native_symbol.to_string();
    let native_icon = chain.native_icon;
    let has_usdt = chain.usdt_address.is_some();
    let from_symbol = token_from().symbol(&chain);

    rsx! {
        div { class: "page swap-page",
            div { class: "topbar",
                button {
                    class: "btn btn-small btn-secondary",
                    onclick: move |_| page.set(Page::Wallet),
                    "← 返回"
                }
                span { class: "topbar-title", "🔄 交换" }
                span {}
            }

            if !chain_supported {
                div { class: "card",
                    p { class: "error", style: "text-align: center;",
                        "⚠️ 当前网络（{chain.short_name}）暂不支持 Uniswap V3 交换。"
                    }
                    p { class: "subtitle", "支持的网络：Ethereum、Arbitrum、Optimism、Base、Polygon" }
                }
            }

            div { class: "card",
                h2 { class: "section-title", "输入代币" }
                div { class: "token-tabs",
                    button {
                        class: if token_from() == SwapToken::Native { "token-tab active" } else { "token-tab" },
                        onclick: move |_| token_from.set(SwapToken::Native),
                        span { class: "token-tab-icon", "{native_icon}" }
                        span { "{native_symbol}" }
                    }
                    button {
                        class: if token_from() == SwapToken::USDC { "token-tab active" } else { "token-tab" },
                        onclick: move |_| token_from.set(SwapToken::USDC),
                        span { class: "token-tab-icon", "🔵" }
                        span { "USDC" }
                    }
                    if has_usdt {
                        button {
                            class: if token_from() == SwapToken::USDT { "token-tab active" } else { "token-tab" },
                            onclick: move |_| token_from.set(SwapToken::USDT),
                            span { class: "token-tab-icon", "🟢" }
                            span { "USDT" }
                        }
                    }
                }

                div { class: "form-group",
                    div { class: "form-label-row",
                        label { "数量 ({from_symbol})" }
                        button {
                            class: "btn-max",
                            disabled: fetching_swap_balance(),
                            onclick: on_set_max_swap,
                            if fetching_swap_balance() { "加载中..." } else { "全部" }
                        }
                    }
                    input {
                        r#type: "text",
                        placeholder: "0.0",
                        value: "{amount_str}",
                        oninput: move |e| {
                            amount_str.set(e.value());
                            // 输入变化时清除报价和存储的最大金额
                            quote_result.set(String::new());
                            quote_amount_out.set(None);
                            max_swap_amount_units.set(None);
                        },
                    }
                }
            }

            // 交换方向按钮
            div { style: "text-align: center; margin: -8px 0;",
                button {
                    class: "btn btn-small btn-secondary",
                    style: "font-size: 18px; padding: 4px 16px; border-radius: 50%;",
                    onclick: move |_| {
                        let f = token_from();
                        let t = token_to();
                        token_from.set(t);
                        token_to.set(f);
                        quote_result.set(String::new());
                        quote_amount_out.set(None);
                    },
                    "⇅"
                }
            }

            div { class: "card",
                h2 { class: "section-title", "输出代币" }
                div { class: "token-tabs",
                    button {
                        class: if token_to() == SwapToken::Native { "token-tab active" } else { "token-tab" },
                        onclick: move |_| token_to.set(SwapToken::Native),
                        span { class: "token-tab-icon", "{native_icon}" }
                        span { "{native_symbol}" }
                    }
                    button {
                        class: if token_to() == SwapToken::USDC { "token-tab active" } else { "token-tab" },
                        onclick: move |_| token_to.set(SwapToken::USDC),
                        span { class: "token-tab-icon", "🔵" }
                        span { "USDC" }
                    }
                    if has_usdt {
                        button {
                            class: if token_to() == SwapToken::USDT { "token-tab active" } else { "token-tab" },
                            onclick: move |_| token_to.set(SwapToken::USDT),
                            span { class: "token-tab-icon", "🟢" }
                            span { "USDT" }
                        }
                    }
                }

                // 报价显示
                if !quote_result().is_empty() {
                    div { class: "quote-display",
                        span { class: "quote-label", "预计获得: " }
                        span { class: "quote-value", "{quote_result}" }
                    }
                }
                if quote_loading() {
                    p { class: "loading-text", "获取报价中..." }
                }
            }

            div { class: "card",
                h2 { class: "section-title", "⚙️ 设置" }

                div { class: "swap-settings-row",
                    div { class: "form-group", style: "flex: 1; margin-right: 8px;",
                        label { "滑点容忍度 (%)" }
                        input {
                            r#type: "text",
                            placeholder: "0.5",
                            value: "{slippage_str}",
                            oninput: move |e| slippage_str.set(e.value()),
                        }
                    }

                    div { class: "form-group", style: "flex: 1; margin-left: 8px;",
                        label { "费率等级" }
                        div { class: "fee-tabs",
                            button {
                                class: if fee_tier() == 500 { "fee-tab active" } else { "fee-tab" },
                                onclick: move |_| {
                                    fee_tier.set(500);
                                    quote_result.set(String::new());
                                    quote_amount_out.set(None);
                                },
                                "0.05%"
                            }
                            button {
                                class: if fee_tier() == 3000 { "fee-tab active" } else { "fee-tab" },
                                onclick: move |_| {
                                    fee_tier.set(3000);
                                    quote_result.set(String::new());
                                    quote_amount_out.set(None);
                                },
                                "0.3%"
                            }
                            button {
                                class: if fee_tier() == 10000 { "fee-tab active" } else { "fee-tab" },
                                onclick: move |_| {
                                    fee_tier.set(10000);
                                    quote_result.set(String::new());
                                    quote_amount_out.set(None);
                                },
                                "1%"
                            }
                        }
                    }
                }
            }

            if !error_msg().is_empty() {
                p { class: "error", style: "margin: 8px 0;", "{error_msg}" }
            }

            if !success_msg().is_empty() {
                div { class: "success-box", style: "margin: 8px 0;",
                    p { "✅ {success_msg}" }
                }
            }

            div { style: "display: flex; gap: 8px; margin-top: 4px;",
                button {
                    class: "btn btn-secondary",
                    style: "flex: 1;",
                    disabled: quote_loading() || !chain_supported,
                    onclick: on_get_quote,
                    if quote_loading() { "查询中..." } else { "📊 获取报价" }
                }
                button {
                    class: "btn btn-primary",
                    style: "flex: 1;",
                    disabled: swapping() || quote_amount_out().is_none() || !chain_supported,
                    onclick: on_swap,
                    if swapping() { "交换中..." } else { "🔄 确认交换" }
                }
            }

            div { class: "notice", style: "margin-top: 12px;",
                "⚠️ 交换通过 Uniswap V3 执行。请确认报价后再执行交换，交易一旦发送无法撤销。"
            }
        }
    }
}

// ============ 链选择模态窗口 ============

#[component]
fn ChainSelectorModal(on_close: EventHandler<()>, on_select: EventHandler<ChainInfo>) -> Element
{
    let chains = all_chains();
    let current_chain_id = SELECTED_CHAIN.read().chain_id;

    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |_| on_close.call(()),

            div {
                class: "modal-content",
                onclick: move |e: Event<MouseData>| e.stop_propagation(),

                h2 { class: "modal-title", "🔗 选择网络" }
                p { class: "modal-subtitle", "选择要使用的区块链网络" }

                div { class: "chain-list",
                    for chain in chains {
                        {
                            let is_active = chain.chain_id == current_chain_id;
                            let chain_for_click = chain.clone();
                            rsx! {
                                button {
                                    class: if is_active { "chain-item active" } else { "chain-item" },
                                    onclick: move |_| on_select.call(chain_for_click.clone()),
                                    span { class: "chain-item-icon", "{chain.chain_icon}" }
                                    div { class: "chain-item-info",
                                        span { class: "chain-item-name", "{chain.name}" }
                                        span { class: "chain-item-detail", "{chain.native_symbol} · Chain ID: {chain.chain_id}" }
                                    }
                                    if is_active {
                                        span { class: "chain-item-check", "✓" }
                                    }
                                }
                            }
                        }
                    }
                }

                button {
                    class: "btn btn-secondary",
                    style: "margin-top: 12px;",
                    onclick: move |_| on_close.call(()),
                    "关闭"
                }
            }
        }
    }
}

// ============ 钱包详情模态窗口 ============

#[component]
fn WalletDetailModal(wallet: WalletInfo, on_close: EventHandler<()>) -> Element
{
    let created_time = format_timestamp(&wallet.created_at.to_string());
    let has_pubkey = !wallet.public_key_x.is_empty() && !wallet.public_key_y.is_empty();

    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |_| on_close.call(()),

            div {
                class: "modal-content wallet-detail-modal",
                onclick: move |e: Event<MouseData>| e.stop_propagation(),

                h2 { class: "modal-title", "📋 钱包详细信息" }
                p { class: "modal-subtitle", "{wallet.name}" }

                div { class: "wallet-detail-list",
                    // 钱包地址
                    div { class: "wallet-detail-row",
                        span { class: "wallet-detail-label", "地址" }
                        span { class: "wallet-detail-value wallet-detail-mono", "{wallet.address}" }
                    }

                    div { class: "wallet-detail-divider" }

                    // 公钥
                    if has_pubkey {
                        div { class: "wallet-detail-row",
                            span { class: "wallet-detail-label", "公钥 X" }
                            span { class: "wallet-detail-value wallet-detail-mono", "{wallet.public_key_x}" }
                        }
                        div { class: "wallet-detail-row",
                            span { class: "wallet-detail-label", "公钥 Y" }
                            span { class: "wallet-detail-value wallet-detail-mono", "{wallet.public_key_y}" }
                        }
                    }
                    if !has_pubkey {
                        div { class: "wallet-detail-row",
                            span { class: "wallet-detail-label", "公钥" }
                            span { class: "wallet-detail-value", style: "color: #888;", "（重新解锁钱包后自动填充）" }
                        }
                    }

                    div { class: "wallet-detail-divider" }

                    // 创建时间
                    div { class: "wallet-detail-row",
                        span { class: "wallet-detail-label", "创建时间" }
                        span { class: "wallet-detail-value", "{created_time}" }
                    }

                    // 钱包 ID
                    div { class: "wallet-detail-row",
                        span { class: "wallet-detail-label", "钱包 ID" }
                        span { class: "wallet-detail-value wallet-detail-mono", style: "font-size: 11px; color: #666;", "{wallet.id}" }
                    }
                }

                button {
                    class: "btn btn-secondary",
                    style: "margin-top: 16px;",
                    onclick: move |_| on_close.call(()),
                    "关闭"
                }
            }
        }
    }
}

// ============ 交易记录组件 ============

#[component]
fn TxItem(tx: TxRecord, my_address: String) -> Element
{
    let mut show_detail = use_signal(|| false);

    let direction_icon = if tx.is_outgoing { "📤" } else { "📥" };
    let direction_class = if tx.is_outgoing { "tx-out" } else { "tx-in" };
    let status_icon = if tx.is_error { "❌" } else { "✅" };
    let counterparty = if tx.is_outgoing
    {
        shorten_address(&tx.to)
    }
    else
    {
        shorten_address(&tx.from)
    };
    let direction_label = if tx.is_outgoing { "发送至" } else { "接收自" };
    let short_hash = shorten_hash(&tx.hash);
    let tx_for_modal = tx.clone();

    rsx! {
        div {
            class: "tx-item tx-item-clickable {direction_class}",
            onclick: move |_| show_detail.set(true),
            div { class: "tx-icon", "{direction_icon}" }
            div { class: "tx-details",
                div { class: "tx-main",
                    span { class: "tx-direction", "{direction_label} " }
                    span { class: "tx-counterparty", title: if tx.is_outgoing { "{tx.to}" } else { "{tx.from}" }, "{counterparty}" }
                }
                div { class: "tx-sub",
                    span { class: "tx-hash", "Tx: {short_hash}" }
                    span { class: "tx-time", "{tx.timestamp}" }
                }
            }
            div { class: "tx-amount",
                span { class: "tx-value", "{tx.value_eth} {SELECTED_CHAIN.read().native_symbol}" }
                span { class: "tx-status", "{status_icon}" }
            }
        }

        if show_detail() {
            TxDetailModal {
                tx: tx_for_modal.clone(),
                on_close: move || show_detail.set(false),
            }
        }
    }
}

// ============ 交易详情模态窗口 ============

#[component]
fn TxDetailModal(tx: TxRecord, on_close: EventHandler<()>) -> Element
{
    let chain = SELECTED_CHAIN.read().clone();
    let explorer_tx_url = format!("{}/tx/{}", chain.explorer_url, tx.hash);
    let status_text = if tx.is_error { "❌ 失败" } else { "✅ 成功" };
    let direction_text = if tx.is_outgoing { "📤 发送" } else { "📥 接收" };
    let explorer_url_clone = explorer_tx_url.clone();

    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |_| on_close.call(()),

            div {
                class: "modal-content tx-detail-modal",
                onclick: move |e: Event<MouseData>| e.stop_propagation(),

                h2 { class: "modal-title", "📋 交易详情" }

                div { class: "tx-detail-list",
                    // 状态
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "状态" }
                        span { class: "tx-detail-value", "{status_text}" }
                    }

                    // 方向
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "类型" }
                        span { class: "tx-detail-value", "{direction_text}" }
                    }

                    // 金额
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "金额" }
                        span { class: "tx-detail-value", "{tx.value_eth} {chain.native_symbol}" }
                    }

                    // Wei 值
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "Wei" }
                        span { class: "tx-detail-value tx-detail-mono", "{tx.value_wei}" }
                    }

                    div { class: "tx-detail-divider" }

                    // 发送方
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "发送方" }
                        span { class: "tx-detail-value tx-detail-mono tx-detail-address", "{tx.from}" }
                    }

                    // 接收方
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "接收方" }
                        span { class: "tx-detail-value tx-detail-mono tx-detail-address", "{tx.to}" }
                    }

                    div { class: "tx-detail-divider" }

                    // 交易哈希
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "交易哈希" }
                        span { class: "tx-detail-value tx-detail-mono tx-detail-address", "{tx.hash}" }
                    }

                    // 区块号
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "区块号" }
                        span { class: "tx-detail-value", "{tx.block_number}" }
                    }

                    // Gas 使用量
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "Gas 用量" }
                        span { class: "tx-detail-value", "{tx.gas_used}" }
                    }

                    // 时间
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "时间" }
                        span { class: "tx-detail-value", "{tx.timestamp}" }
                    }
                }
                // 在区块浏览器中查看按钮
                button {
                    class: "btn btn-primary",
                    style: "margin-top: 16px;",
                    r#type: "button",
                    onclick: move |_| {},
                    // 使用内联 JavaScript 打开外部浏览器（WebView 支持）
                    "onmousedown": format!("window.open('{}', '_system');", explorer_url_clone),
                    "🔗 在区块浏览器中查看"
                }

                button {
                    class: "btn btn-secondary",
                    style: "margin-top: 8px;",
                    onclick: move |_| on_close.call(()),
                    "关闭"
                }
            }
        }
    }
}

// ============ 全局状态 ============

static UNLOCKED_SK: dioxus::prelude::GlobalSignal<Option<String>> = GlobalSignal::new(|| None);
static CURRENT_WALLET_ID: dioxus::prelude::GlobalSignal<Option<String>> = GlobalSignal::new(|| None);
static ETHERSCAN_KEY: dioxus::prelude::GlobalSignal<String> = GlobalSignal::new(|| load_api_key());
static SELECTED_CHAIN: dioxus::prelude::GlobalSignal<ChainInfo> = GlobalSignal::new(|| default_chain());

// ============ 辅助函数 ============

// ============ 钱包列表管理 ============

/// 获取数据根目录（Android 数据目录）
fn get_data_dir() -> Option<std::path::PathBuf>
{
    let cmdline = std::fs::read("/proc/self/cmdline").ok()?;
    let package_name = String::from_utf8_lossy(&cmdline)
        .trim_matches(char::from(0))
        .to_string();
    if package_name.is_empty() {
        return None;
    }
    let path = std::path::PathBuf::from(format!("/data/data/{}", package_name));
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// 获取钱包 keystore 文件的完整路径: <exe_dir>/wallets/<id>
fn get_wallet_keystore_path(wallet_id: &str) -> std::path::PathBuf
{
    let dir = get_data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    dir.join(WALLETS_DIR).join(format!("{}", wallet_id))
}

/// 获取 wallet_list.json 的完整路径（放在 wallets 文件夹中）
fn get_wallet_list_path() -> std::path::PathBuf
{
    let dir = get_data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let wallets_dir = dir.join(WALLETS_DIR);
    // 确保 wallets 目录存在
    let _ = std::fs::create_dir_all(&wallets_dir);
    wallets_dir.join(WALLET_LIST_FILE)
}

/// 读取钱包列表；文件不存在则返回空列表
fn load_wallet_list() -> WalletList
{
    let path = get_wallet_list_path();
    if path.exists()
    {
        if let Ok(data) = std::fs::read_to_string(&path)
        {
            if let Ok(list) = serde_json::from_str::<WalletList>(&data)
            {
                return list;
            }
        }
    }
    WalletList {
        wallets: Vec::new(),
        default_wallet_id: None,
    }
}

/// 保存钱包列表到 wallet_list.json
fn save_wallet_list(list: &WalletList)
{
    let path = get_wallet_list_path();
    if let Ok(data) = serde_json::to_string_pretty(list)
    {
        let _ = std::fs::write(path, data);
    }
}

/// 生成唯一钱包 ID（基于公钥 x 坐标的前 8 字节）
/// 
/// # 参数
/// - `private_key_hex`: 私钥的十六进制字符串
/// 
/// # 返回
/// - 钱包 ID（16 位十六进制字符，即公钥 x 坐标的前 8 字节）
fn generate_wallet_id(private_key_hex: &str) -> Result<String, String>
{
    // 获取公钥坐标
    let (x_hex, _y_hex) = blockchain::account::private_key_hex_to_public_key_xy(private_key_hex)?;
    
    // 取 x 坐标的前 8 字节（16 个十六进制字符）
    let id = x_hex.chars().take(16).collect::<String>();
    
    Ok(id)
}

/// 获取当前 Unix 时间戳（秒）
fn current_unix_timestamp() -> u64
{
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn load_api_key() -> String
{
    if let Some(dir) = get_data_dir()
    {
        let api_dir = dir.join("api");
        if let Ok(key) = std::fs::read_to_string(api_dir.join(API_KEY_FILE))
        {
            let key = key.trim().to_string();
            if !key.is_empty()
            {
                return key;
            }
        }
    }
    String::new()
}

fn save_api_key(key: &str)
{
    if let Some(dir) = get_data_dir()
    {
        let api_dir = dir.join("api");
        // 确保 api 目录存在
        let _ = std::fs::create_dir_all(&api_dir);
        let _ = std::fs::write(api_dir.join(API_KEY_FILE), key.trim());
    }
}

fn shorten_address(addr: &str) -> String
{
    if addr.len() > 12
    {
        format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
    }
    else
    {
        addr.to_string()
    }
}

fn shorten_hash(hash: &str) -> String
{
    if hash.len() > 16
    {
        format!("{}...{}", &hash[..10], &hash[hash.len() - 6..])
    }
    else
    {
        hash.to_string()
    }
}

fn format_timestamp(unix_str: &str) -> String
{
    if let Ok(ts) = unix_str.parse::<i64>()
    {
        // 转换为 UTC+8 时间（东八区）
        let utc8_offset = 8 * 3600; // 8小时的秒数
        let secs = ts + utc8_offset;
        let days_since_epoch = secs / 86400;
        let time_of_day = secs % 86400;
        let hours = time_of_day / 3600;
        let minutes = (time_of_day % 3600) / 60;

        let (year, month, day) = days_to_ymd(days_since_epoch);
        format!("{:04}-{:02}-{:02} {:02}:{:02} (UTC+8)", year, month, day, hours, minutes)
    }
    else
    {
        unix_str.to_string()
    }
}

fn days_to_ymd(days: i64) -> (i64, i64, i64)
{
    let mut y = 1970;
    let mut remaining = days;

    loop
    {
        let days_in_year = if is_leap_year(y) { 366 } else { 365 };
        if remaining < days_in_year
        {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }

    let months_days = if is_leap_year(y)
    {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    }
    else
    {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 1;
    for &md in &months_days
    {
        if remaining < md
        {
            break;
        }
        remaining -= md;
        m += 1;
    }

    (y, m, remaining + 1)
}

fn is_leap_year(y: i64) -> bool
{
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn wei_decimal_to_eth(wei_str: &str) -> String
{
    if let Ok(wei) = wei_str.parse::<u128>()
    {
        let eth = blockchain::utils::wei_to_ether(wei);
        if eth == 0.0
        {
            "0".to_string()
        }
        else if eth < 0.0001
        {
            format!("{:.6}", eth)
        }
        else
        {
            format!("{:.4}", eth)
        }
    }
    else
    {
        "0".to_string()
    }
}

// ============ 异步数据获取 ============

async fn fetch_token_balance(address: &str, token: &TokenType, chain: &ChainInfo) -> Result<String, String>
{
    let rpc = blockchain::rpc::RpcClient::new(chain.rpc_url);

    match token.contract_address(chain)
    {
        None =>
        {
            // 原生代币：直接查余额（返回 Wei）
            let balance_hex = rpc.eth_get_balance(address, "latest").await?;
            let balance_wei = blockchain::utils::hex_to_u128(&balance_hex)?;
            
            if balance_wei == 0
            {
                Ok("0".to_string())
            }
            else
            {
                // 格式化为人类可读的字符串（18 位小数）
                let display_str = format_units_to_decimal(balance_wei, 18);
                Ok(display_str)
            }
        }
        Some(contract_addr) =>
        {
            // ERC20：通过合约查余额
            let balance_hex = blockchain::contract::get_erc20_balance(&rpc, &contract_addr, address).await?;
            let balance_u128 = blockchain::utils::hex_to_u128(&balance_hex)?;
            let decimals = token.decimals(chain) as u32;
            let divisor = 10u128.pow(decimals);
            let whole = balance_u128 / divisor;
            let frac = balance_u128 % divisor;
            if balance_u128 == 0
            {
                Ok("0".to_string())
            }
            else
            {
                // 显示小数位数与代币精度匹配（USDC/USDT 为 6 位）
                let frac_str = format!("{:0>width$}", frac, width = decimals as usize);
                let trimmed = frac_str.trim_end_matches('0');
                if trimmed.is_empty()
                {
                    Ok(format!("{}", whole))
                }
                else
                {
                    Ok(format!("{}.{}", whole, trimmed))
                }
            }
        }
    }
}

/// 获取代币余额（返回最小单位，u128）
async fn fetch_token_balance_units(address: &str, token: &TokenType, chain: &ChainInfo) -> Result<u128, String>
{
    let rpc = blockchain::rpc::RpcClient::new(chain.rpc_url);

    match token.contract_address(chain)
    {
        None =>
        {
            // 原生代币：直接查余额（返回 Wei）
            let balance_hex = rpc.eth_get_balance(address, "latest").await?;
            let balance_units = blockchain::utils::hex_to_u128(&balance_hex)?;
            Ok(balance_units)
        }
        Some(contract_addr) =>
        {
            // ERC20：通过合约查余额（返回最小单位）
            let balance_hex = blockchain::contract::get_erc20_balance(&rpc, &contract_addr, address).await?;
            let balance_units = blockchain::utils::hex_to_u128(&balance_hex)?;
            Ok(balance_units)
        }
    }
}

/// 将最小单位金额格式化为人类可读的十进制字符串
fn format_units_to_decimal(units: u128, decimals: u8) -> String
{
    let divisor = 10u128.pow(decimals as u32);
    let whole = units / divisor;
    let frac = units % divisor;
    
    if frac == 0 {
        format!("{}", whole)
    } else {
        // 格式化小数部分，去除尾部的零
        let frac_str = format!("{:0>width$}", frac, width = decimals as usize);
        let trimmed = frac_str.trim_end_matches('0');
        format!("{}.{}", whole, trimmed)
    }
}

async fn fetch_transactions(address: &str, chain: &ChainInfo) -> Result<Vec<TxRecord>, String>
{
    let api_key = ETHERSCAN_KEY.read().clone();
    if api_key.is_empty()
    {
        return Err("需要 Etherscan API Key 才能查看交易历史。请在下方设置中填入你的 API Key（可在 etherscan.io 免费注册获取）".to_string());
    }

    let client = blockchain::etherscan::EtherscanClient::new(chain.explorer_api_url, &api_key, chain.chain_id);
    let txs = client.get_recent_transactions(address, 20).await?;

    let my_addr = address.to_lowercase();
    let records: Vec<TxRecord> = txs
        .into_iter()
        .map(|tx| {
            let is_outgoing = tx.from.to_lowercase() == my_addr;
            TxRecord {
                hash: tx.hash,
                from: tx.from,
                to: tx.to,
                value_eth: wei_decimal_to_eth(&tx.value),
                value_wei: tx.value,
                timestamp: format_timestamp(&tx.timestamp),
                block_number: tx.block_number,
                gas_used: tx.gas_used,
                is_error: tx.is_error == "1",
                is_outgoing,
            }
        })
        .collect();
    Ok(records)
}

/// 解析代币金额字符串为最小单位（纯整数，不经过浮点数）
/// 
/// # 参数
/// - `amount_str`: 金额字符串，如 "10.5" 或 "10"
/// - `decimals`: 代币精度（小数位数），如 18 或 6
/// 
/// # 返回
/// - Ok(u128): 最小单位数量
/// - Err(String): 错误信息
fn parse_token_amount_to_units(amount_str: &str, decimals: u8) -> Result<u128, String> {
    if amount_str.is_empty() {
        return Err("请输入金额".to_string());
    }

    if amount_str.contains('.') {
        // 有小数点的情况
        let parts: Vec<&str> = amount_str.split('.').collect();
        if parts.len() != 2 {
            return Err("金额格式无效".to_string());
        }
        
        // 解析整数部分
        let integer_part: u128 = parts[0].parse()
            .map_err(|_| "金额格式无效".to_string())?;
        
        // 处理小数部分
        let mut decimal_part = parts[1].to_string();
        if decimal_part.len() > decimals as usize {
            return Err(format!("最多支持 {} 位小数", decimals));
        }
        
        // 补齐到指定精度
        while decimal_part.len() < decimals as usize {
            decimal_part.push('0');
        }
        
        let decimal_value: u128 = decimal_part.parse()
            .map_err(|_| "金额格式无效".to_string())?;
        
        // 计算最小单位：整数部分 * 10^decimals + 小数部分
        let divisor = 10u128.pow(decimals as u32);
        Ok(integer_part * divisor + decimal_value)
    } else {
        // 没有小数点，直接解析整数部分
        let integer_part: u128 = amount_str.parse()
            .map_err(|_| "金额格式无效".to_string())?;
        let divisor = 10u128.pow(decimals as u32);
        Ok(integer_part * divisor)
    }
}

/// 发送原生代币交易（ETH、BNB、MATIC 等）- 使用最小单位
async fn send_native_transaction_units(
    from: &str,
    to: &str,
    amount_units: u128,
    sk_hex: &str,
    chain: &ChainInfo,
    gas_fee_percent: u32,
) -> Result<String, String>
{
    let rpc = blockchain::rpc::RpcClient::new(chain.rpc_url);
    let chain_id = chain.chain_id;

    // 1. 获取 nonce
    let nonce = rpc.eth_get_transaction_count(from, "latest").await?;

    // 2. 获取 gas price 并按百分比上浮
    let gas_price_hex = rpc.eth_gas_price().await?;
    let base_gas_price = blockchain::utils::hex_to_u128(&gas_price_hex)?;
    let gas_price = base_gas_price + base_gas_price * gas_fee_percent as u128 / 100;

    // 3. amount_units 已经是 Wei（最小单位）
    let tx = blockchain::transaction::TransactionBuilder::new()
        .tx_type(blockchain::transaction::TxType::Legacy)
        .chain_id(chain_id)
        .nonce(nonce)
        .to(to)
        .value_u128(amount_units)
        .gas_limit(21000)
        .gas_price_u128(gas_price)
        .build()?;

    let signed = blockchain::transaction::sign_transaction_with_hex(&tx, sk_hex)?;
    let raw_hex = blockchain::transaction::serialize_signed_transaction(&signed);
    let tx_hash = rpc.eth_send_raw_transaction(&raw_hex).await?;
    Ok(tx_hash)
}

/// 发送 ERC20 代币交易（USDC、USDT 等）
/// amount_units: 代币最小单位数量（USDC/USDT 为 10^6）
async fn send_erc20_transaction(
    from: &str,
    to: &str,
    amount_units: u128,
    contract_address: &str,
    sk_hex: &str,
    chain: &ChainInfo,
    gas_fee_percent: u32,
) -> Result<String, String>
{
    let rpc = blockchain::rpc::RpcClient::new(chain.rpc_url);
    let chain_id = chain.chain_id;

    // 1. 获取 nonce
    let nonce = rpc.eth_get_transaction_count(from, "latest").await?;

    // 2. 获取 gas price 并按百分比上浮
    let gas_price_hex = rpc.eth_gas_price().await?;
    let base_gas_price = blockchain::utils::hex_to_u128(&gas_price_hex)?;
    let gas_price = base_gas_price + base_gas_price * gas_fee_percent as u128 / 100;

    // 3. 编码 ERC20 transfer 调用
    let amount_hex = format!("0x{:x}", amount_units);
    let data = blockchain::contract::encode_erc20_transfer(to, &amount_hex)?;
    let data_hex = format!("0x{}", blockchain::utils::bytes_to_hex(&data));

    // 4. 估算 gas
    let estimate_tx = serde_json::json!({
        "from": from,
        "to": contract_address,
        "data": data_hex
    });
    let gas_estimate = rpc.eth_estimate_gas(estimate_tx).await?;
    // 加 20% 余量
    let gas_limit = gas_estimate + gas_estimate / 5;

    let tx = blockchain::transaction::TransactionBuilder::new()
        .tx_type(blockchain::transaction::TxType::Legacy)
        .chain_id(chain_id)
        .nonce(nonce)
        .to(contract_address)
        .value_u128(0) // ERC20 转账 value 为 0
        .data(data)
        .gas_limit(gas_limit)
        .gas_price_u128(gas_price)
        .build()?;

    let signed = blockchain::transaction::sign_transaction_with_hex(&tx, sk_hex)?;
    let raw_hex = blockchain::transaction::serialize_signed_transaction(&signed);
    let tx_hash = rpc.eth_send_raw_transaction(&raw_hex).await?;
    Ok(tx_hash)
}

// ============ CSS 样式 ============

const CSS: &str = r#"
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
    background: #0f0f1a;
    color: #e0e0e0;
    min-height: 100vh;
}

.app-container {
    width: 100%;
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
}

.page {
    width: 100%;
    max-width: 100%;
    margin: 0 auto;
    padding: 16px;
    min-height: 100vh;
}

.login-page, .import-page {
    display: flex;
    align-items: center;
    justify-content: center;
}

.card {
    background: #1a1a2e;
    border-radius: 16px;
    padding: 20px;
    margin-bottom: 16px;
    border: 1px solid #2a2a4a;
    width: 100%;
}

.title {
    font-size: 28px;
    font-weight: 700;
    text-align: center;
    margin-bottom: 8px;
    background: linear-gradient(135deg, #667eea, #764ba2);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
}

.subtitle {
    text-align: center;
    color: #888;
    margin-bottom: 24px;
    font-size: 14px;
}

.form-group {
    margin-bottom: 16px;
}

.form-group label {
    display: block;
    font-size: 13px;
    color: #aaa;
    margin-bottom: 6px;
    font-weight: 500;
}

.form-label-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 6px;
}

.form-label-row label {
    margin-bottom: 0;
}

.btn-max {
    padding: 6px 16px;
    background: #1a1a2e;
    border: 1px solid #333;
    border-radius: 8px;
    color: #667eea;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
}

.btn-max:hover:not(:disabled) {
    border-color: #667eea;
    background: rgba(102, 126, 234, 0.1);
}

.btn-max:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}

.form-group input {
    width: 100%;
    padding: 12px 16px;
    background: #0f0f1a;
    border: 1px solid #333;
    border-radius: 10px;
    color: #e0e0e0;
    font-size: 14px;
    outline: none;
    transition: border-color 0.2s;
}

.form-group input:focus {
    border-color: #667eea;
}

.form-group input::placeholder {
    color: #555;
}

.gas-fee-buttons {
    display: flex;
    gap: 10px;
    margin-top: 8px;
}

.gas-fee-btn {
    flex: 1;
    padding: 10px 16px;
    background: #1a1a2e;
    border: 1px solid #333;
    border-radius: 8px;
    color: #aaa;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
}

.gas-fee-btn:hover {
    border-color: #667eea;
    color: #e0e0e0;
}

.gas-fee-btn.active {
    background: linear-gradient(135deg, #667eea, #764ba2);
    border-color: #667eea;
    color: white;
    box-shadow: 0 2px 10px rgba(102, 126, 234, 0.3);
}

.gas-fee-hint {
    font-size: 12px;
    color: #777;
    margin-top: 8px;
    line-height: 1.4;
}

.error {
    color: #ff6b6b;
    font-size: 13px;
    margin-bottom: 12px;
    padding: 8px 12px;
    background: rgba(255, 107, 107, 0.1);
    border-radius: 8px;
    border: 1px solid rgba(255, 107, 107, 0.2);
}

.btn {
    display: block;
    width: 100%;
    padding: 12px;
    border: none;
    border-radius: 10px;
    font-size: 15px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
    text-align: center;
}

.btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}

.btn-primary {
    background: linear-gradient(135deg, #667eea, #764ba2);
    color: white;
}

.btn-primary:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 4px 15px rgba(102, 126, 234, 0.4);
}

.btn-secondary {
    background: transparent;
    color: #667eea;
    border: 1px solid #667eea;
}

.btn-secondary:hover:not(:disabled) {
    background: rgba(102, 126, 234, 0.1);
}

.btn-small {
    width: auto;
    padding: 6px 14px;
    font-size: 13px;
}

.btn-danger {
    background: rgba(255, 107, 107, 0.15);
    color: #ff6b6b;
    border: 1px solid rgba(255, 107, 107, 0.3);
}

.btn-danger:hover:not(:disabled) {
    background: rgba(255, 107, 107, 0.25);
}

.btn-action {
    width: auto;
    flex: 1;
    padding: 10px 8px;
    background: rgba(102, 126, 234, 0.1);
    color: #667eea;
    border: 1px solid rgba(102, 126, 234, 0.2);
    font-size: 13px;
}

.btn-action:hover:not(:disabled) {
    background: rgba(102, 126, 234, 0.2);
}

.divider {
    height: 1px;
    background: #2a2a4a;
    margin: 16px 0;
}

.topbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
    padding: 8px 0;
}

.topbar-title {
    font-size: 18px;
    font-weight: 700;
    background: linear-gradient(135deg, #667eea, #764ba2);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
}

.token-tabs {
    display: flex;
    gap: 6px;
    margin-bottom: 12px;
}

.token-tab {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 10px 8px;
    border-radius: 10px;
    border: 1px solid #2a2a4a;
    background: #1a1a2e;
    color: #888;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
}

.token-tab:hover {
    border-color: #667eea;
    color: #ccc;
}

.token-tab.active {
    border-color: #667eea;
    background: rgba(102, 126, 234, 0.12);
    color: #fff;
}

.token-tab-icon {
    font-size: 16px;
}

.account-card {
    text-align: center;
}

.account-address {
    margin-bottom: 16px;
}

.account-address .label {
    display: block;
    font-size: 12px;
    color: #888;
    margin-bottom: 4px;
}

.account-address .address {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 14px;
    color: #aaa;
    cursor: default;
}

.account-balance {
    margin-bottom: 20px;
}

.balance-value {
    font-size: 36px;
    font-weight: 700;
    color: #fff;
    margin-right: 8px;
}

.balance-unit {
    font-size: 18px;
    color: #888;
}

.action-buttons {
    display: flex;
    gap: 8px;
}

.tx-card {
    max-height: 400px;
    overflow-y: auto;
}

.section-title {
    font-size: 16px;
    font-weight: 600;
    margin-bottom: 12px;
    color: #ccc;
}

.loading-text, .empty-text {
    color: #666;
    text-align: center;
    padding: 20px;
    font-size: 14px;
}

.tx-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.tx-item {
    display: flex;
    align-items: center;
    padding: 12px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.02);
    transition: background 0.2s;
    gap: 12px;
}

.tx-item:hover {
    background: rgba(255, 255, 255, 0.05);
}

.tx-icon {
    font-size: 20px;
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    flex-shrink: 0;
}

.tx-out .tx-icon { background: rgba(255, 107, 107, 0.15); }
.tx-in .tx-icon { background: rgba(107, 255, 107, 0.15); }

.tx-details {
    flex: 1;
    min-width: 0;
}

.tx-main {
    font-size: 14px;
    margin-bottom: 2px;
}

.tx-direction {
    color: #aaa;
}

.tx-counterparty {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 12px;
    color: #ccc;
}

.tx-sub {
    font-size: 11px;
    color: #666;
    display: flex;
    gap: 8px;
}

.tx-hash {
    font-family: "SF Mono", "Fira Code", monospace;
}

.tx-amount {
    text-align: right;
    flex-shrink: 0;
}

.tx-value {
    display: block;
    font-size: 14px;
    font-weight: 600;
    color: #fff;
}

.tx-out .tx-value { color: #ff6b6b; }
.tx-in .tx-value { color: #6bff6b; }

.tx-status {
    font-size: 11px;
}

.future-card {
    opacity: 0.6;
}

.future-features {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
}

.feature-tag {
    padding: 6px 12px;
    background: rgba(102, 126, 234, 0.08);
    border: 1px solid rgba(102, 126, 234, 0.15);
    border-radius: 20px;
    font-size: 12px;
    color: #888;
}

::-webkit-scrollbar {
    width: 6px;
}

::-webkit-scrollbar-track {
    background: transparent;
}

::-webkit-scrollbar-thumb {
    background: #333;
    border-radius: 3px;
}

::-webkit-scrollbar-thumb:hover {
    background: #444;
}

/* ======== 发送页面 ======== */

.send-page .topbar,
.receive-page .topbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
    padding: 8px 0;
}

.success-box {
    background: rgba(107, 255, 107, 0.08);
    border: 1px solid rgba(107, 255, 107, 0.25);
    border-radius: 10px;
    padding: 14px 16px;
    margin-bottom: 14px;
    text-align: center;
}

.success-box p {
    color: #6bff6b;
    font-size: 14px;
    margin-bottom: 4px;
}

.tx-hash-display {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 11px;
    color: #aaa;
    word-break: break-all;
    margin-top: 6px;
}

.notice {
    margin-top: 14px;
    padding: 10px 14px;
    background: rgba(255, 200, 50, 0.08);
    border: 1px solid rgba(255, 200, 50, 0.2);
    border-radius: 8px;
    font-size: 12px;
    color: #bba84f;
    text-align: center;
}

/* ======== 接收页面 ======== */

.address-display {
    background: #0f0f1a;
    border: 1px solid #333;
    border-radius: 10px;
    padding: 18px 16px;
    margin: 16px 0 0 0;
    text-align: center;
    word-break: break-all;
}

.full-address {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 15px;
    color: #e0e0e0;
    letter-spacing: 0.5px;
    line-height: 1.6;
}

.copy-hint {
    color: #6bff6b;
    font-size: 13px;
    text-align: center;
    margin-top: 10px;
}

/* ======== 链选择器 ======== */

.chain-selector-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 10px 16px;
    margin-bottom: 10px;
    border-radius: 10px;
    border: 1px solid #2a2a4a;
    background: #1a1a2e;
    color: #ccc;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
}

.chain-selector-btn:hover {
    border-color: #667eea;
    background: rgba(102, 126, 234, 0.08);
}

.chain-selector-icon {
    font-size: 18px;
}

.chain-selector-name {
    flex: 1;
    text-align: left;
}

.chain-selector-arrow {
    color: #667eea;
    font-size: 12px;
}

/* ======== 模态窗口 ======== */

.modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.65);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
}

.modal-content {
    background: #1a1a2e;
    border: 1px solid #2a2a4a;
    border-radius: 16px;
    padding: 20px;
    width: 95%;
    max-width: 100%;
    max-height: 80vh;
    overflow-y: auto;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
}

.modal-title {
    font-size: 20px;
    font-weight: 700;
    text-align: center;
    margin-bottom: 4px;
    color: #e0e0e0;
}

.modal-subtitle {
    text-align: center;
    color: #888;
    font-size: 13px;
    margin-bottom: 16px;
}

.chain-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.chain-item {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 12px 14px;
    border-radius: 10px;
    border: 1px solid transparent;
    background: rgba(255, 255, 255, 0.02);
    color: #ccc;
    font-size: 14px;
    cursor: pointer;
    transition: all 0.2s;
    text-align: left;
}

.chain-item:hover {
    background: rgba(102, 126, 234, 0.08);
    border-color: rgba(102, 126, 234, 0.3);
}

.chain-item.active {
    background: rgba(102, 126, 234, 0.12);
    border-color: #667eea;
}

.chain-item-icon {
    font-size: 22px;
    width: 32px;
    text-align: center;
    flex-shrink: 0;
}

.chain-item-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
}

.chain-item-name {
    font-weight: 600;
    font-size: 14px;
    color: #e0e0e0;
}

.chain-item-detail {
    font-size: 11px;
    color: #666;
    margin-top: 2px;
}

.chain-item-check {
    color: #667eea;
    font-weight: 700;
    font-size: 16px;
    flex-shrink: 0;
}

/* ======== 交易详情 ======== */

.tx-item-clickable {
    cursor: pointer;
}

.tx-detail-modal {
    max-width: 100%;
}

.tx-detail-list {
    display: flex;
    flex-direction: column;
    gap: 0;
}

.tx-detail-row {
    display: flex;
    flex-direction: column;
    padding: 10px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
}

.tx-detail-row:last-child {
    border-bottom: none;
}

.tx-detail-label {
    font-size: 11px;
    color: #888;
    margin-bottom: 4px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.5px;
}

.tx-detail-value {
    font-size: 14px;
    color: #e0e0e0;
    line-height: 1.4;
}

.tx-detail-mono {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 12px;
}

.tx-detail-address {
    word-break: break-all;
    color: #ccc;
}

.tx-detail-divider {
    height: 1px;
    background: #2a2a4a;
    margin: 4px 0;
}

/* ======== 钱包列表页面 ======== */

.wallet-list-page {
    display: flex;
    flex-direction: column;
    padding-top: 20px;
}

.wallet-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
}

.wallet-list-item {
    border: 1px solid #2a2a4a;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.02);
    overflow: hidden;
    transition: all 0.2s;
}

.wallet-list-item:hover {
    border-color: rgba(102, 126, 234, 0.4);
    background: rgba(102, 126, 234, 0.04);
}

.wallet-list-item.default {
    border-color: rgba(102, 126, 234, 0.4);
    background: rgba(102, 126, 234, 0.06);
}

.wallet-list-main {
    display: flex;
    align-items: center;
    padding: 14px 16px;
    cursor: pointer;
    gap: 12px;
}

.wallet-list-info {
    flex: 1;
    min-width: 0;
}

.wallet-list-name {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 15px;
    font-weight: 600;
    color: #e0e0e0;
    margin-bottom: 4px;
}

.wallet-default-badge {
    font-size: 10px;
    font-weight: 600;
    color: #667eea;
    background: rgba(102, 126, 234, 0.15);
    border: 1px solid rgba(102, 126, 234, 0.3);
    border-radius: 4px;
    padding: 1px 6px;
}

.wallet-list-addr {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 12px;
    color: #888;
}

.wallet-list-arrow {
    color: #667eea;
    font-size: 16px;
    flex-shrink: 0;
}

.wallet-list-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 16px 12px 16px;
}

.delete-confirm-text {
    font-size: 12px;
    color: #ff6b6b;
    margin-right: 4px;
}

/* ======== 钱包改名 ======== */

.wallet-rename-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 16px 8px 16px;
}

.wallet-rename-input {
    flex: 1;
    padding: 6px 10px;
    background: #0f0f1a;
    border: 1px solid #667eea;
    border-radius: 6px;
    color: #e0e0e0;
    font-size: 13px;
    outline: none;
}

.wallet-rename-input::placeholder {
    color: #555;
}

/* ======== 钱包选择器（主页下拉） ======== */

.wallet-selector-container {
    position: relative;
    margin-bottom: 10px;
}

.wallet-selector-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 10px 16px;
    border-radius: 10px;
    border: 1px solid #2a2a4a;
    background: #1a1a2e;
    color: #ccc;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
}

.wallet-selector-btn:hover {
    border-color: #667eea;
    background: rgba(102, 126, 234, 0.08);
}

.wallet-selector-icon {
    font-size: 18px;
}

.wallet-selector-name {
    flex: 1;
    text-align: left;
}

.wallet-selector-arrow {
    color: #667eea;
    font-size: 12px;
}

.wallet-dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    z-index: 100;
    background: #1a1a2e;
    border: 1px solid #2a2a4a;
    border-top: none;
    border-radius: 0 0 12px 12px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    max-height: 360px;
    overflow-y: auto;
}

.wallet-dropdown-item {
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    transition: background 0.2s;
}

.wallet-dropdown-item:last-child {
    border-bottom: none;
}

.wallet-dropdown-item:hover {
    background: rgba(102, 126, 234, 0.06);
}

.wallet-dropdown-item.current {
    background: rgba(102, 126, 234, 0.10);
}

.wallet-dropdown-main {
    display: flex;
    align-items: center;
    padding: 12px 16px 4px 16px;
    cursor: pointer;
    gap: 10px;
}

.wallet-dropdown-info {
    flex: 1;
    min-width: 0;
}

.wallet-dropdown-name {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 14px;
    font-weight: 600;
    color: #e0e0e0;
    margin-bottom: 2px;
}

.wallet-current-badge {
    font-size: 10px;
    font-weight: 600;
    color: #6bff6b;
    background: rgba(107, 255, 107, 0.12);
    border: 1px solid rgba(107, 255, 107, 0.25);
    border-radius: 4px;
    padding: 1px 6px;
}

.wallet-dropdown-addr {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 11px;
    color: #888;
}

.wallet-dropdown-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 6px;
    padding: 6px 16px 10px 16px;
}

.wallet-dropdown-footer {
    padding: 10px 16px;
    border-top: 1px solid #2a2a4a;
    display: flex;
    justify-content: center;
}

/* ======== 钱包详情模态窗口 ======== */

.wallet-detail-modal {
    max-width: 440px;
}

.wallet-detail-list {
    display: flex;
    flex-direction: column;
    gap: 0;
}

.wallet-detail-row {
    display: flex;
    flex-direction: column;
    padding: 10px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
}

.wallet-detail-row:last-child {
    border-bottom: none;
}

.wallet-detail-label {
    font-size: 11px;
    color: #888;
    margin-bottom: 4px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.5px;
}

.wallet-detail-value {
    font-size: 14px;
    color: #e0e0e0;
    line-height: 1.4;
}

.wallet-detail-mono {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 12px;
    word-break: break-all;
    color: #ccc;
}

.wallet-detail-divider {
    height: 1px;
    background: #2a2a4a;
    margin: 4px 0;
}

/* ---- 交换页面 ---- */

.quote-display {
    margin-top: 12px;
    padding: 12px 16px;
    background: #16213e;
    border-radius: 10px;
    border: 1px solid #2a2a4a;
    text-align: center;
}

.quote-label {
    font-size: 13px;
    color: #888;
}

.quote-value {
    font-size: 20px;
    font-weight: 700;
    color: #6bff6b;
    margin-left: 4px;
}

.swap-settings-row {
    display: flex;
    align-items: flex-start;
}

.fee-tabs {
    display: flex;
    gap: 4px;
}

.fee-tab {
    flex: 1;
    padding: 6px 8px;
    border-radius: 8px;
    border: 1px solid #2a2a4a;
    background: transparent;
    color: #999;
    font-size: 12px;
    cursor: pointer;
    transition: all 0.2s;
    text-align: center;
}

.fee-tab:hover {
    border-color: #667eea;
    color: #ccc;
}

.fee-tab.active {
    background: #667eea33;
    border-color: #667eea;
    color: #667eea;
    font-weight: 600;
}
"#;