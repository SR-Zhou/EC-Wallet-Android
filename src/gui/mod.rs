use crate::blockchain;
use crate::crypto;
use crate::data::{
    self, BALANCE_CACHE, ChainInfo, RequestPriority, TX_CACHE, TX_SERIAL, TokenType, TxRecord,
    all_chains, balance_key, default_chain, tx_cache_key,
};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use zeroize::Zeroize;

// ============ 常量 ============
const WALLETS_DIR: &str = "wallets";
const WALLET_LIST_FILE: &str = "wallet_list.json";

// ============ 应用状态 ============

/// 应用页面
#[derive(Debug, Clone, PartialEq)]
enum Page {
    /// 钱包列表页面（选择/管理钱包）
    WalletList,
    /// 导入钱包页面
    Import,
    /// 主钱包页面
    Wallet,
    /// 钱包资产页（底部资产 tab）
    WalletAssets,
    /// 发送页面
    Send,
    /// 接收页面
    Receive,
    /// 交换页面
    Swap,
    /// 交易详情页面
    TransactionDetail(TxRecord),
    /// 资产详情页面（链+代币的余额和交易记录）
    AssetDetail {
        chain_id: u64,
        chain_name: String,
        token: TokenType,
        symbol: String,
        enter_anim: bool,
    },
}

/// 待执行的操作（签名后自动执行）
#[derive(Debug, Clone)]
enum PendingOp {
    None,
    Send {
        recipient: String,
        amount_units: u128,
        amount_str: String,
        token: TokenType,
        chain: ChainInfo,
    },
    Swap {
        token_in_addr: String,
        token_out_addr: String,
        amount_in: u128,
        amount_str: String,
        amount_out_min: u128,
        fee: u32,
        recipient: String,
        chain_id: u64,
        rpc_url: String,
    },
}

// ============ 钱包列表数据结构 ============

/// 单个钱包信息（存储在 wallet_list.json 中）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct WalletInfo {
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
struct WalletList {
    pub wallets: Vec<WalletInfo>,
    pub default_wallet_id: Option<String>,
}

// ============ 入口 ============

pub fn launch() {
    dioxus::launch(app);
}

// ============ 主组件 ============

fn app() -> Element {
    use_hook(|| {
        data::start_request_worker();
    });

    let page = use_signal(|| {
        let list = load_wallet_list();
        if list.wallets.is_empty() {
            Page::Import
        } else if let Some(ref default_id) = list.default_wallet_id {
            *CURRENT_WALLET_ID.write() = Some(default_id.clone());
            Page::Wallet
        } else {
            Page::WalletList
        }
    });
    let page_stack = use_signal(|| Vec::<Page>::new());
    rsx! {
        style { {CSS} }
        div { class: "app-container",
            match page() {
                Page::WalletList => rsx! { WalletListPage { page } },
                Page::Import => rsx! { ImportPage { page } },
                Page::Wallet => rsx! { WalletPage { page, page_stack, initial_tab: HomeTab::Home } },
                Page::WalletAssets => rsx! { WalletPage { page, page_stack, initial_tab: HomeTab::Assets } },
                Page::Send => rsx! { SendPage { page, page_stack } },
                Page::Receive => rsx! { ReceivePage { page, page_stack } },
                Page::Swap => rsx! { SwapPage { page, page_stack } },
                Page::TransactionDetail(tx) => rsx! { TransactionDetailPage { page, page_stack, tx } },
                Page::AssetDetail { chain_id, chain_name, token, symbol, enter_anim } => rsx! { AssetDetailPage { page, page_stack, chain_id, chain_name, token, symbol, enter_anim } },
            }
        }
    }
}

// ============ 导入钱包页面 ============

#[component]
fn ImportPage(page: Signal<Page>) -> Element {
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
        if sk.is_empty() {
            error_msg.set("请输入私钥".into());
            return;
        }
        if name.is_empty() {
            error_msg.set("请输入钱包名称".into());
            return;
        }
        if pw.is_empty() {
            error_msg.set("请输入密码".into());
            return;
        }
        if pw != pw2 {
            error_msg.set("两次密码不一致".into());
            return;
        }

        let sk_clean = if sk.starts_with("0x") || sk.starts_with("0X") {
            sk[2..].to_string()
        } else {
            sk.clone()
        };

        if !blockchain::conversion::is_valid_private_key(&sk_clean) {
            error_msg.set("私钥格式无效".into());
            return;
        }

        // 计算地址
        let address = match blockchain::account::private_key_hex_to_address(&sk_clean) {
            Ok(addr) => addr,
            Err(e) => {
                error_msg.set(format!("私钥无效: {}", e));
                return;
            }
        };

        loading.set(true);
        error_msg.set(String::new());

        // 生成钱包 ID
        let wallet_id = match generate_wallet_id(&sk_clean) {
            Ok(id) => id,
            Err(e) => {
                error_msg.set(format!("生成钱包 ID 失败: {}", e));
                loading.set(false);
                return;
            }
        };
        let wallet_name = name;

        // 存储 keystore 文件
        let keystore_path = get_wallet_keystore_path(&wallet_id);
        match crypto::keystore::store(pw, sk_clean.clone(), &keystore_path, false) {
            Ok(()) => {
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
                if is_first {
                    list.default_wallet_id = Some(wallet_id.clone());
                }
                save_wallet_list(&list);

                *CURRENT_WALLET_ID.write() = Some(wallet_id);
                page.set(Page::Wallet);
            }
            Err(e) => {
                error_msg.set(format!("导入失败: {}", e));
            }
        }
        loading.set(false);
    };

    let go_login = move |_| {
        let list = load_wallet_list();
        if !list.wallets.is_empty() {
            if let Some(ref default_id) = list.default_wallet_id {
                *CURRENT_WALLET_ID.write() = Some(default_id.clone());
                page.set(Page::Wallet);
            } else {
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
fn WalletListPage(page: Signal<Page>) -> Element {
    let mut wallet_list = use_signal(|| load_wallet_list());
    let mut confirm_delete: Signal<Option<String>> = use_signal(|| None);
    let mut show_wallet_detail: Signal<Option<WalletInfo>> = use_signal(|| None);
    let mut renaming_id: Signal<Option<String>> = use_signal(|| None);
    let mut rename_input = use_signal(|| String::new());

    let mut on_select = move |id: String| {
        *CURRENT_WALLET_ID.write() = Some(id.clone());
        page.set(Page::Wallet);
    };

    let mut on_set_default = move |id: String| {
        let mut list = wallet_list();
        list.default_wallet_id = Some(id);
        save_wallet_list(&list);
        wallet_list.set(list);
    };

    let mut on_delete = move |id: String| {
        let mut list = wallet_list();
        delete_wallet(&id, &mut list);
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
                p { class: "subtitle", style: "text-align: left; margin-bottom: 16px;", "选择要使用的钱包，或导入新钱包" }

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
                                                    } div {
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

#[derive(Debug, Clone, PartialEq)]
enum HomeTab {
    Home,
    Assets,
}

#[component]
fn WalletPage(
    page: Signal<Page>,
    page_stack: Signal<Vec<Page>>,
    initial_tab: HomeTab,
) -> Element {
    let address = get_current_wallet_address();
    let mut selected_token = use_signal(|| TokenType::Native);
    let mut show_chain_dropdown = use_signal(|| false);
    let mut show_wallet_dropdown = use_signal(|| false);
    let mut show_wallet_detail: Signal<Option<WalletInfo>> = use_signal(|| None);
    let mut renaming_id: Signal<Option<String>> = use_signal(|| None);
    let mut rename_input = use_signal(|| String::new());
    let mut confirm_delete: Signal<Option<String>> = use_signal(|| None);
    let mut wallet_list_version = use_signal(|| 0u32); // 用于触发钱包列表刷新
    let mut balance = use_signal(|| "加载中...".to_string());
    let mut balance_error = use_signal(|| String::new());
    let mut transactions: Signal<Vec<TxRecord>> = use_signal(Vec::new);
    let mut tx_loading = use_signal(|| false);
    let mut tx_error = use_signal(|| String::new());
    let refresh_counter = use_signal(|| 0u32);
    let mut balance_refresh_counter = use_signal(|| 0u32);
    let mut tx_refresh_counter = use_signal(|| 0u32);
    let mut current_tab = use_signal(move || initial_tab);

    // 钱包缓存由 data 层激活；钱包切换时它会加载对应缓存并丢弃旧页面的结果。
    use_effect(move || {
        data::activate_wallet(CURRENT_WALLET_ID.read().clone());
    });

    // 余额随 selected_token / selected_chain / refresh 变化而获取（优先使用缓存）
    let address_for_balance = address.clone();
    use_effect(move || {
        if current_tab() != HomeTab::Home {
            return;
        }
        let _ = refresh_counter();
        let _ = balance_refresh_counter();
        let addr = address_for_balance.clone();
        let token = selected_token().clone();
        let chain = SELECTED_CHAIN.read().clone();
        let cache_key = balance_key(chain.chain_id, &token);

        if let Some(cached) = BALANCE_CACHE.read().get(&cache_key) {
            match cached {
                Ok(bal) => {
                    balance_error.set(String::new());
                    balance.set(truncate_balance_for_display(bal));
                }
                Err(e) => {
                    balance.set(String::new());
                    balance_error.set(format!("获取余额失败: {}", e));
                }
            }
            return;
        }

        balance_error.set(String::new());
        balance.set("加载中...".to_string());
        if let Some(wallet_id) = CURRENT_WALLET_ID.read().clone() {
            data::enqueue_balance(&wallet_id, &addr, chain, token, RequestPriority::High);
        }
    });

    // 交易记录随 selected_chain / selected_token / refresh 变化而获取（优先使用缓存）
    use_effect(move || {
        if current_tab() != HomeTab::Home {
            return;
        }
        let _ = *TX_SERIAL.read();
        let _ = refresh_counter();
        let _ = tx_refresh_counter();
        let addr = get_current_wallet_address();
        let chain = SELECTED_CHAIN.read().clone();
        let token = selected_token().clone();
        let chain_id = chain.chain_id;
        let cache_key = tx_cache_key(chain_id, &token);

        if let Some(cached) = TX_CACHE.read().get(&cache_key) {
            match cached {
                Ok(txs) => {
                    tx_error.set(String::new());
                    transactions.set(txs.clone());
                    tx_loading.set(false);
                }
                Err(e) => {
                    transactions.set(Vec::new());
                    tx_error.set(format!("获取交易历史失败: {}", e));
                    tx_loading.set(false);
                }
            }
            return;
        }

        transactions.set(Vec::new());
        tx_loading.set(true);
        tx_error.set(String::new());
        if let Some(wallet_id) = CURRENT_WALLET_ID.read().clone() {
            data::enqueue_transactions(&wallet_id, &addr, chain, RequestPriority::High);
        }
    });

    let addr_display = address.clone();
    let short_addr = shorten_address(&addr_display);
    let chain = SELECTED_CHAIN.read().clone();
    let current_chain_id = chain.chain_id;
    let native_symbol = chain.native_symbol.to_string();
    let native_icon = chain.native_icon;
    let chain_display_name = chain.short_name.to_string();
    let chain_icon = chain.chain_icon;
    let has_usdt = chain.usdt_address.is_some();

    rsx! {
        div { class: "page wallet-page",
            div { class: "topbar",
                span { class: "topbar-title", "🔷 EC Wallet" }
                span {}
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
                            span { class: "wallet-selector-icon", "📋" }
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
                                                            // 切换到该钱包
                                                            *CURRENT_WALLET_ID.write() = Some(w_id.clone());
                                                            page.set(Page::Wallet);
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
                                                                        delete_wallet(&del_id, &mut list);
                                                                        // 如果删除的是当前钱包，返回钱包列表页面
                                                                        if del_id == curr_wid {
                                                                            *CURRENT_WALLET_ID.write() = None;
                                                                            page.set(Page::WalletList);
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
                                        class: "btn btn-small btn-secondary",
                                        onclick: move |_| {
                                            show_wallet_dropdown.set(false);
                                            page.set(Page::WalletList);
                                        },
                                        "钱包列表"
                                    }
                                    button {
                                        class: "btn btn-small btn-secondary",
                                        onclick: move |_| {
                                            show_wallet_dropdown.set(false);
                                            page.set(Page::Import);
                                        },
                                        "导入新钱包"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ---- 链选择器 ----下拉 ----
            div { class: "chain-selector-container",
                button {
                    class: "chain-selector-btn",
                    onclick: move |_| show_chain_dropdown.set(!show_chain_dropdown()),
                    span { class: "chain-selector-icon", "{chain_icon}" }
                    span { class: "chain-selector-name", "{chain_display_name}" }
                    span { class: "chain-selector-arrow", if show_chain_dropdown() { "▴" } else { "▾" } }
                }

                if show_chain_dropdown() {
                    div { class: "chain-dropdown",
                        for chain in all_chains() {
                            {
                                let is_active = chain.chain_id == current_chain_id;
                                let chain_for_click = chain.clone();
                                rsx! {
                                    button {
                                        class: if is_active { "chain-item active" } else { "chain-item" },
                                        onclick: move |_| {
                                            *SELECTED_CHAIN.write() = chain_for_click.clone();
                                            selected_token.set(TokenType::Native);
                                            show_chain_dropdown.set(false);
                                        },
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
                }
            }

            if current_tab() == HomeTab::Home {
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
                div { class: "account-header",
                    span { class: "section-title", "余额" }
                    button {
                        class: "btn btn-small btn-secondary",
                        onclick: move |_| {
                            let chain = SELECTED_CHAIN.read().clone();
                            let token = selected_token();
                            if let Some(ref wid) = *CURRENT_WALLET_ID.read() {
                                data::clear_balance(wid, chain.chain_id, &token);
                            }
                            balance_refresh_counter.set(balance_refresh_counter() + 1);
                        },
                        "刷新"
                    }
                }
                div { class: "account-address",
                    span { class: "address", title: "{addr_display}", "{short_addr}" }
                }
                if !balance_error().is_empty() {
                    p { class: "error", "{balance_error}" }
                } else {
                    div { class: "account-balance",
                        span { class: "balance-value", "{balance}" }
                        span { class: "balance-unit", "{selected_token().symbol(&chain)}" }
                    }
                }
                div { class: "action-buttons",
                    button {
                        class: "btn btn-action",
                        onclick: move |_| {
                            page_stack.write().push(Page::Wallet);
                            page.set(Page::Send);
                        },
                        "发送"
                    }
                    button {
                        class: "btn btn-action",
                        onclick: move |_| {
                            page_stack.write().push(Page::Wallet);
                            page.set(Page::Receive);
                        },
                        "接收"
                    }
                    button {
                        class: "btn btn-action",
                        onclick: move |_| {
                            page_stack.write().push(Page::Wallet);
                            page.set(Page::Swap);
                        },
                        "交换"
                    }
                }
            }

            div { class: "card tx-card",
                div { class: "tx-header",
                    h2 { class: "section-title", "交易历史" }
                    button {
                        class: "btn btn-small btn-secondary",
                        onclick: move |_| {
                            let chain = SELECTED_CHAIN.read().clone();
                            if let Some(ref wid) = *CURRENT_WALLET_ID.read() {
                                data::clear_chain_transactions(wid, chain.chain_id);
                            }
                            tx_refresh_counter.set(tx_refresh_counter() + 1);
                        },
                        "刷新"
                    }
                }

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
                        TxItem { tx: tx.clone(), my_address: addr_display.clone(), page, page_stack }
                    }
                }
            }
            }

            if current_tab() == HomeTab::Assets {
                AssetsPage { address: address.clone(), refresh_counter, current_tab, page, page_stack }
            }

            // ---- 钱包详情模态窗口 ----
            if let Some(detail_wallet) = show_wallet_detail() {
                WalletDetailModal {
                    wallet: detail_wallet,
                    on_close: move || show_wallet_detail.set(None),
                }
            }
        }
        // 底部导航栏
        div { class: "bottom-bar",
            button {
                class: if current_tab() == HomeTab::Home { "bottom-bar-btn active" } else { "bottom-bar-btn" },
                onclick: move |_| current_tab.set(HomeTab::Home),
                "主页"
            }
            button {
                class: if current_tab() == HomeTab::Assets { "bottom-bar-btn active" } else { "bottom-bar-btn" },
                onclick: move |_| current_tab.set(HomeTab::Assets),
                "资产"
            }
        }
    }
}

// ============ 资产页面 ============

#[derive(Debug, Clone, PartialEq)]
struct AssetItem {
    chain_id: u64,
    chain_name: String,
    token: TokenType,
    symbol: String,
    balance_str: String,
    is_zero: bool,
}

/// 预定义调色板（最多支持 30 种代币-链组合）
const ASSET_COLORS: &[&str] = &[
    "#667eea", "#f093fb", "#4facfe", "#43e97b", "#fa709a", "#fee140", "#30cfd0", "#a8c0ff",
    "#f86ca7", "#ff6b6b", "#f9d423", "#00b4db", "#f5576c", "#005bea", "#48c6ef", "#6bff6b",
    "#f9a826", "#a18cd1", "#ffecd2", "#fcb69f", "#cfd9df", "#b24592", "#e14b5d", "#f3a183",
    "#4a00e0", "#8e2de2", "#00bf8f", "#11998e", "#38ef7d", "#ffb347",
];

#[component]
fn AssetsPage(
    address: String,
    refresh_counter: Signal<u32>,
    current_tab: Signal<HomeTab>,
    page: Signal<Page>,
    page_stack: Signal<Vec<Page>>,
) -> Element {
    let mut assets = use_signal(|| Vec::<AssetItem>::new());
    let mut loading = use_signal(|| true);
    let mut serial_trigger = use_signal(|| 0u32);

    // 收集所有链 × 代币的数据（读取缓存），并触发串行加载
    use_effect(move || {
        if current_tab() != HomeTab::Assets {
            return;
        }
        let _ = refresh_counter();
        let _ = serial_trigger();
        loading.set(true);
        let chains = all_chains();
        let mut items = Vec::new();
        let mut need_fetch = Vec::new();

        for chain in &chains {
            let tokens = vec![TokenType::Native, TokenType::USDC, TokenType::USDT];
            for token in &tokens {
                if *token == TokenType::USDC && chain.usdc_address.is_none() {
                    continue;
                }
                if *token == TokenType::USDT && chain.usdt_address.is_none() {
                    continue;
                }

                let cache_key = balance_key(chain.chain_id, &token);
                if let Some(cached) = BALANCE_CACHE.read().get(&cache_key) {
                    match cached {
                        Ok(bal) => {
                            let is_zero = bal == "0";
                            items.push(AssetItem {
                                chain_id: chain.chain_id,
                                chain_name: chain.name.to_string(),
                                token: token.clone(),
                                symbol: token.symbol(chain).to_string(),
                                balance_str: bal.clone(),
                                is_zero,
                            });
                        }
                        Err(e) => {
                            items.push(AssetItem {
                                chain_id: chain.chain_id,
                                chain_name: chain.name.to_string(),
                                token: token.clone(),
                                symbol: token.symbol(chain).to_string(),
                                balance_str: format!("错误: {}", e),
                                is_zero: false,
                            });
                        }
                    }
                } else {
                    need_fetch.push((chain.clone(), token.clone()));
                    items.push(AssetItem {
                        chain_id: chain.chain_id,
                        chain_name: chain.name.to_string(),
                        token: token.clone(),
                        symbol: token.symbol(chain).to_string(),
                        balance_str: "加载中...".to_string(),
                        is_zero: false,
                    });
                }
            }
        }

        assets.set(items);

        // 资产页只入队。余额是正常优先级，交易历史预取为低优先级。
        loading.set(!need_fetch.is_empty());
        if let Some(wallet_id) = CURRENT_WALLET_ID.read().clone() {
            for (chain, token) in need_fetch {
                data::enqueue_balance(&wallet_id, &address, chain, token, RequestPriority::Normal);
            }

            for chain in chains {
                if chain.blockscout_api_url.is_some()
                    && !TX_CACHE
                        .read()
                        .contains_key(&tx_cache_key(chain.chain_id, &TokenType::Native))
                {
                    data::enqueue_transactions(&wallet_id, &address, chain, RequestPriority::Low);
                }
            }
        }
    });

    // 排序：链顺序 → token顺序(Native,USDC,USDT) → 非0在前
    let all_chains_list = all_chains();
    let chain_order: HashMap<u64, usize> = all_chains_list
        .iter()
        .enumerate()
        .map(|(i, c)| (c.chain_id, i))
        .collect();
    let token_order: HashMap<TokenType, usize> = vec![
        (TokenType::Native, 0),
        (TokenType::USDC, 1),
        (TokenType::USDT, 2),
    ]
    .into_iter()
    .collect();

    let mut sorted = assets();
    sorted.sort_by(|a, b| {
        let chain_a = chain_order.get(&a.chain_id).copied().unwrap_or(99);
        let chain_b = chain_order.get(&b.chain_id).copied().unwrap_or(99);
        if chain_a != chain_b {
            return chain_a.cmp(&chain_b);
        }
        let tok_a = token_order.get(&a.token).copied().unwrap_or(99);
        let tok_b = token_order.get(&b.token).copied().unwrap_or(99);
        if tok_a != tok_b {
            return tok_a.cmp(&tok_b);
        }
        // 非0在前
        b.is_zero.cmp(&a.is_zero)
    });

    // 过滤：只显示有余额的资产（排除零余额、错误、加载中）
    let filtered: Vec<&AssetItem> = sorted
        .iter()
        .filter(|item| {
            !item.is_zero
                && !item.balance_str.starts_with("错误")
                && item.balance_str != "加载中..."
        })
        .collect();

    // 环图数据（仅非0且非错误）
    let ring_data: Vec<(usize, &AssetItem, f64)> = {
        let non_zero: Vec<&AssetItem> = sorted
            .iter()
            .filter(|item| {
                !item.is_zero
                    && !item.balance_str.starts_with("错误")
                    && item.balance_str != "加载中..."
            })
            .collect();
        let total: f64 = non_zero
            .iter()
            .filter_map(|item| item.balance_str.parse::<f64>().ok())
            .sum();
        if total > 0.0 {
            non_zero
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| {
                    item.balance_str
                        .parse::<f64>()
                        .ok()
                        .map(|val| (idx, *item, val / total))
                })
                .collect()
        } else {
            Vec::new()
        }
    };

    let ring_cx = 180.0;
    let ring_cy = 160.0;
    let ring_r = 100.0;
    let ring_inner = 55.0;

    // 弧段路径
    fn arc_path(cx: f64, cy: f64, r: f64, inner: f64, start_angle: f64, end_angle: f64) -> String {
        let sweep = end_angle - start_angle;
        // 处理 360° 全圆（SVG 弧起止点相同会什么都不画）
        if sweep.abs() >= 2.0 * std::f64::consts::PI - 0.001 {
            let mid = start_angle + std::f64::consts::PI;
            let first = arc_path_half(cx, cy, r, inner, start_angle, mid);
            let second = arc_path_half(cx, cy, r, inner, mid, end_angle);
            return format!("{} {}", first, second);
        }
        arc_path_half(cx, cy, r, inner, start_angle, end_angle)
    }

    fn arc_path_half(
        cx: f64,
        cy: f64,
        r: f64,
        inner: f64,
        start_angle: f64,
        end_angle: f64,
    ) -> String {
        let outer_x1 = cx + r * start_angle.cos();
        let outer_y1 = cy + r * start_angle.sin();
        let outer_x2 = cx + r * end_angle.cos();
        let outer_y2 = cy + r * end_angle.sin();
        let inner_x1 = cx + inner * end_angle.cos();
        let inner_y1 = cy + inner * end_angle.sin();
        let inner_x2 = cx + inner * start_angle.cos();
        let inner_y2 = cy + inner * start_angle.sin();
        let large = (end_angle - start_angle).abs() > std::f64::consts::PI;
        format!(
            "M {} {} A {} {} 0 {} 1 {} {} L {} {} A {} {} 0 {} 0 {} {} Z",
            outer_x1,
            outer_y1,
            r,
            r,
            large as u8,
            outer_x2,
            outer_y2,
            inner_x1,
            inner_y1,
            inner,
            inner,
            large as u8,
            inner_x2,
            inner_y2
        )
    }

    let rsx_svg = if ring_data.is_empty() {
        rsx! {
            text { x: "{ring_cx}", y: "{ring_cy}", text_anchor: "middle", dominant_baseline: "middle",
                font_size: "14", fill: "#999", "暂无资产" }
        }
    } else {
        let mut angle = -std::f64::consts::PI / 2.0;
        let mut paths = Vec::new();
        for (idx, item, ratio) in ring_data.iter() {
            let sweep = 2.0 * std::f64::consts::PI * ratio;
            let start_angle = angle;
            let end_angle = angle + sweep;
            let color = ASSET_COLORS[idx % ASSET_COLORS.len()];
            let path = arc_path(ring_cx, ring_cy, ring_r, ring_inner, start_angle, end_angle);

            let path_d = path;
            let color_s = color.to_string();
            let chain_id = item.chain_id;
            let chain_name = item.chain_name.clone();
            let token = item.token.clone();
            let symbol = item.symbol.clone();
            let mut p = page;
            let mut stack = page_stack;

            paths.push(rsx! {
                path {
                    d: "{path_d}",
                    fill: "{color_s}",
                    opacity: "0.85",
                    stroke: "#fff",
                    stroke_width: "1",
                    style: "cursor: pointer;",
                    onclick: move |_| {
                        stack.write().push(Page::WalletAssets);
                        p.set(Page::AssetDetail { chain_id, chain_name: chain_name.clone(), token: token.clone(), symbol: symbol.clone(), enter_anim: true });
                    },
                }
            });
            angle += sweep;
        }

        rsx! {
            for path_elem in paths {
                {path_elem}
            }
        }
    };

    // 图例数据：按环中顺序，去重
    let mut legend: Vec<(String, String)> = Vec::new();
    let mut seen = HashSet::new();
    for (_idx, item, _ratio) in &ring_data {
        let label = format!("{} ({})", item.symbol, item.chain_name);
        if seen.insert(label.clone()) {
            // 用 ring_data 的索引对应颜色
            let idx = legend.len();
            let c = ASSET_COLORS[idx % ASSET_COLORS.len()].to_string();
            legend.push((c, label));
        }
    }

    rsx! {
        div { class: "assets-page",
            div { class: "assets-ring-container",
                div { class: "assets-ring-header",
                    span {} // placeholder for flex
                    button {
                        class: "btn btn-small btn-secondary",
                        onclick: move |_| {
                            let chain = SELECTED_CHAIN.read().clone();
                            if let Some(ref wid) = *CURRENT_WALLET_ID.read() {
                                data::clear_chain_balances(wid, chain.chain_id);
                                data::clear_chain_transactions(wid, chain.chain_id);
                            }
                            serial_trigger.set(serial_trigger() + 1);
                        },
                        "刷新"
                    }
                }
                if loading() {
                    div { class: "assets-loading",
                        div { class: "assets-spinner" }
                        p { class: "loading-text", "加载资产中..." }
                    }
                } else {
                    svg {
                        view_box: "0 0 360 280",
                        width: "100%",
                        height: "auto",
                        {rsx_svg}
                    }
                    // 图例（环下方，居中）
                    if !legend.is_empty() {
                        div { class: "ring-legend",
                            for (color, label) in &legend {
                                div { class: "ring-legend-item",
                                    span {
                                        class: "ring-legend-color",
                                        style: "background: {color};",
                                    }
                                    span { class: "ring-legend-label", "{label}" }
                                }
                            }
                        }
                    }
                }
            }

            div { class: "asset-list",
                for item in filtered {
                    {
                        let chain_id = item.chain_id;
                        let chain_name = item.chain_name.clone();
                        let token = item.token.clone();
                        let symbol = item.symbol.clone();
                        let mut p = page;
                        let mut stack = page_stack;
                        rsx! {
                            div {
                                class: "asset-item",
                                onclick: move |_| {
                                    stack.write().push(Page::WalletAssets);
                                    p.set(Page::AssetDetail { chain_id, chain_name: chain_name.clone(), token: token.clone(), symbol: symbol.clone(), enter_anim: true });
                                },
                                div { class: "asset-item-left",
                                    span { class: "asset-item-symbol", "{item.symbol} ({item.chain_name})" }
                                }
                                div { class: "asset-item-right",
                                    span { class: "asset-item-balance", "{item.balance_str}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ============ 登录面板（SendPage / SwapPage 共用）============

#[component]
fn LoginSheet(
    visible: Signal<bool>,
    leaving: Signal<bool>,
    password: Signal<String>,
    error: Signal<String>,
    loading: Signal<bool>,
) -> Element {
    let wallet_list = load_wallet_list();
    let current_wid = CURRENT_WALLET_ID.read().clone().unwrap_or_default();
    let wallet_info = wallet_list.wallets.iter().find(|w| w.id == current_wid).cloned();
    let sheet_name = wallet_info.as_ref().map(|w| w.name.clone()).unwrap_or_default();
    let sheet_addr = wallet_info.as_ref().map(|w| shorten_address(&w.address)).unwrap_or_default();
    let sheet_wid = current_wid;

    rsx! {
        if visible() {
            div {
                class: "login-overlay",
                onclick: {
                    move |_| {
                        leaving.set(true);
                        *PENDING_OP.write() = PendingOp::None;
                    }
                },
                div {
                    class: if leaving() { "card login-sheet leaving" } else { "card login-sheet" },
                    onclick: move |e: Event<MouseData>| e.stop_propagation(),
                    onanimationend: move |_: Event<AnimationData>| {
                        if leaving() {
                            visible.set(false);
                            leaving.set(false);
                        }
                    },
                    h1 { class: "title", "🔐 EC Wallet" }
                    p { class: "subtitle", "签名: {sheet_name}" }
                    if !sheet_addr.is_empty() {
                        p { class: "subtitle", style: "margin-top: -16px; font-family: monospace; font-size: 12px; color: #999;", "{sheet_addr}" }
                    }
                    div { class: "form-group",
                        label { "密码" }
                        input {
                            r#type: "password",
                            placeholder: "请输入密码...",
                            value: "{password}",
                            oninput: move |e| password.set(e.value()),
                            onkeypress: {
                                let wid = sheet_wid.clone();
                                move |e: Event<KeyboardData>| {
                                    if e.key() == Key::Enter {
                                        let pw = password().trim().to_string();
                                        if pw.is_empty() {
                                            error.set("请输入密码".into());
                                            return;
                                        }
                                        loading.set(true);
                                        error.set(String::new());
                                        let keystore_path = get_wallet_keystore_path(&wid);
                                        match crypto::keystore::load(pw, &keystore_path, false) {
                                            Ok(sk_hex) => {
                                                UNLOCKED_SK.write().replace(sk_hex);
                                                leaving.set(true);
                                            }
                                            Err(e) => {
                                                error.set(format!("解锁失败: {}", e));
                                            }
                                        }
                                        loading.set(false);
                                    }
                                }
                            },
                        }
                    }
                    if !error().is_empty() {
                        p { class: "error", "{error}" }
                    }
                    button {
                        class: "btn btn-primary",
                        disabled: loading(),
                        onclick: {
                            let wid = sheet_wid.clone();
                            move |_: Event<MouseData>| {
                                let pw = password().trim().to_string();
                                if pw.is_empty() {
                                    error.set("请输入密码".into());
                                    return;
                                }
                                loading.set(true);
                                error.set(String::new());
                                let keystore_path = get_wallet_keystore_path(&wid);
                                match crypto::keystore::load(pw, &keystore_path, false) {
                                    Ok(sk_hex) => {
                                        UNLOCKED_SK.write().replace(sk_hex);
                                        leaving.set(true);
                                    }
                                    Err(e) => {
                                        error.set(format!("解锁失败: {}", e));
                                    }
                                }
                                loading.set(false);
                            }
                        },
                        if loading() { "解锁中..." } else { "🔓 解锁并签名" }
                    }
                }
            }
        }
    }
}

// ============ 发送页面 ============

#[component]
fn SendPage(page: Signal<Page>, page_stack: Signal<Vec<Page>>) -> Element {
    let address = get_current_wallet_address();

    let mut selected_token = use_signal(|| TokenType::Native);
    let mut recipient = use_signal(|| String::new());
    let mut amount_str = use_signal(|| String::new());
    let mut error_msg = use_signal(|| String::new());
    let mut success_msg = use_signal(|| String::new());
    let mut sending = use_signal(|| false);
    let mut fetching_balance = use_signal(|| false);
    let mut max_amount_units: Signal<Option<u128>> = use_signal(|| None); // 存储精确的最小单位余额

    let mut login_visible = use_signal(|| false);
    let mut login_password = use_signal(|| String::new());
    let mut login_error = use_signal(|| String::new());
    let login_loading = use_signal(|| false);
    let login_leaving = use_signal(|| false);

    let my_addr = address.clone();
    let my_addr_for_max = my_addr.clone(); // 用于 on_set_max

    // 若有待执行的发送操作，恢复表单数据
    use_effect(move || {
        if let PendingOp::Send {
            recipient: ref rec,
            amount_str: ref amt,
            token: ref tok,
            ..
        } = PENDING_OP.read().clone()
        {
            recipient.set(rec.clone());
            amount_str.set(amt.clone());
            selected_token.set(tok.clone());
        }
    });

    // 点击"全部"按钮的处理函数
    let on_set_max = move |_| {
        let token = selected_token();
        let addr = my_addr_for_max.clone();
        let chain = SELECTED_CHAIN.read().clone();

        fetching_balance.set(true);
        error_msg.set(String::new());

        spawn(async move {
            // 获取余额（返回最小单位）
            let contract_address = token.contract_address(&chain);
            let balance_units = match blockchain::read::fetch_balance_units(
                &addr,
                chain.rpc_url,
                contract_address.as_deref(),
            )
            .await
            {
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
            let display_str =
                blockchain::read::format_units_to_decimal(balance_units, decimals, None);
            amount_str.set(display_str);

            fetching_balance.set(false);
        });
    };

    let address_for_send = address.clone();
    let on_send = move |_| {
        let to_addr = recipient().trim().to_string();
        let amount_input = amount_str().trim().to_string();
        let token = selected_token();
        let chain = SELECTED_CHAIN.read().clone();

        // 基本验证
        if to_addr.is_empty() {
            error_msg.set("请输入接收地址".into());
            return;
        }
        if !blockchain::utils::is_valid_address(&to_addr) {
            error_msg.set("接收地址格式无效".into());
            return;
        }
        if amount_input.is_empty() {
            error_msg.set("请输入金额".into());
            return;
        }

        // 解析金额
        let stored_max = max_amount_units();
        let amount_units = if let Some(max_units) = stored_max {
            max_units
        } else {
            match token {
                TokenType::Native => match parse_token_amount_to_units(&amount_input, 18) {
                    Ok(v) => v,
                    Err(e) => {
                        error_msg.set(e);
                        return;
                    }
                },
                TokenType::USDC | TokenType::USDT => {
                    let decimals = token.decimals(&chain);
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

        if amount_units == 0 {
            error_msg.set("金额必须大于零".into());
            return;
        }

        // 检查私钥
        if UNLOCKED_SK.read().is_none() {
            // 暂存操作，显示登录面板
            *PENDING_OP.write() = PendingOp::Send {
                recipient: to_addr,
                amount_units,
                amount_str: amount_input,
                token: token.clone(),
                chain,
            };
            login_password.set(String::new());
            login_error.set(String::new());
            login_visible.set(true);
            return;
        }

        let sk_hex = UNLOCKED_SK.read().clone().unwrap();
        let from_addr = address_for_send.clone();

        sending.set(true);
        error_msg.set(String::new());
        success_msg.set(String::new());
        let is_max = if let Some(max_units) = stored_max {
            amount_units == max_units
        } else {
            false
        };

        spawn(async move {
            let result = match token {
                TokenType::Native => {
                    blockchain::transfer::send_native_transaction_units(
                        &from_addr,
                        &to_addr,
                        amount_units,
                        &sk_hex,
                        chain.chain_id,
                        chain.rpc_url,
                        is_max,
                    )
                    .await
                }
                TokenType::USDC | TokenType::USDT => {
                    let contract_addr = match token.contract_address(&chain) {
                        Some(addr) => addr,
                        None => {
                            error_msg.set(format!("该链不支持 {}", token.symbol(&chain)));
                            sending.set(false);
                            if let Some(mut sk) = UNLOCKED_SK.write().take() {
                                sk.zeroize();
                            }
                            return;
                        }
                    };
                    blockchain::transfer::send_erc20_transaction(
                        &from_addr,
                        &to_addr,
                        amount_units,
                        &contract_addr,
                        &sk_hex,
                        chain.chain_id,
                        chain.rpc_url,
                    )
                    .await
                }
            };

            match result {
                Ok(tx_hash) => {
                    success_msg.set(format!("交易已发送！交易哈希: {}", tx_hash));
                    amount_str.set(String::new());
                    max_amount_units.set(None);
                    *PENDING_OP.write() = PendingOp::None;
                }
                Err(e) => {
                    error_msg.set(format!("发送失败: {}", e));
                }
            }
            sending.set(false);
            if let Some(mut sk) = UNLOCKED_SK.write().take() {
                sk.zeroize();
            }
        });
    };

    // 当解锁完成后自动执行暂存的操作
    use_effect(move || {
        let op = PENDING_OP.read().clone();
        match op {
            PendingOp::Send {
                recipient,
                amount_units,
                token,
                chain,
                ..
            } => {
                if let Some(sk_hex) = UNLOCKED_SK.read().clone() {
                    let from_addr = address.clone();
                    sending.set(true);
                    error_msg.set(String::new());
                    success_msg.set(String::new());
                    let is_max = max_amount_units.read().is_some();
                    spawn(async move {
                        let result = match token {
                            TokenType::Native => {
                                blockchain::transfer::send_native_transaction_units(
                                    &from_addr,
                                    &recipient,
                                    amount_units,
                                    &sk_hex,
                                    chain.chain_id,
                                    chain.rpc_url,
                                    is_max,
                                )
                                .await
                            }
                            TokenType::USDC | TokenType::USDT => {
                                let contract_addr = match token.contract_address(&chain) {
                                    Some(addr) => addr,
                                    None => {
                                        error_msg
                                            .set(format!("该链不支持 {}", token.symbol(&chain)));
                                        sending.set(false);
                                        if let Some(mut sk) = UNLOCKED_SK.write().take() {
                                            sk.zeroize();
                                        }
                                        return;
                                    }
                                };
                                blockchain::transfer::send_erc20_transaction(
                                    &from_addr,
                                    &recipient,
                                    amount_units,
                                    &contract_addr,
                                    &sk_hex,
                                    chain.chain_id,
                                    chain.rpc_url,
                                )
                                .await
                            }
                        };
                        match result {
                            Ok(tx_hash) => {
                                success_msg.set(format!("交易已发送！交易哈希: {}", tx_hash));
                                amount_str.set(String::new());
                                max_amount_units.set(None);
                                *PENDING_OP.write() = PendingOp::None;
                            }
                            Err(e) => {
                                error_msg.set(format!("发送失败: {}", e));
                            }
                        }
                        sending.set(false);
                        if let Some(mut sk) = UNLOCKED_SK.write().take() {
                            sk.zeroize();
                        }
                    });
                }
            }
            _ => {}
        }
    });

    let mut leave_anim = use_signal(|| String::new());
    let mut touch_start = use_signal(|| (0.0f64, 0.0f64));

    let go_back = move || {
        *PENDING_OP.write() = PendingOp::None;
        let mut stack = page_stack;
        let mut p = page;
        if let Some(prev) = stack.write().pop() {
            p.set(prev);
        }
    };

    let on_touch_start = move |evt: Event<TouchData>| {
        if let Some(touch) = evt.touches().first() {
            let coords = touch.client_coordinates();
            touch_start.set((coords.x, coords.y));
        }
    };

    let on_touch_end = move |evt: Event<TouchData>| {
        let (start_x, start_y) = touch_start();
        if let Some(touch) = evt.touches_changed().first() {
            let coords = touch.client_coordinates();
            let dx = coords.x - start_x;
            let dy = coords.y - start_y;
            if dx.abs() > 100.0 && dx.abs() > dy.abs() {
                if dx > 0.0 {
                    leave_anim.set("page-leave-right".into());
                } else {
                    leave_anim.set("page-leave-left".into());
                }
            }
        }
    };

    let on_animation_end = move |_: Event<AnimationData>| {
        if !leave_anim().is_empty() {
            go_back();
        }
    };

    let chain = SELECTED_CHAIN.read().clone();
    let native_symbol = chain.native_symbol.to_string();
    let native_icon = chain.native_icon;
    let has_usdt = chain.usdt_address.is_some();
    let token_symbol_title = selected_token().symbol(&chain);
    let token_symbol_label = selected_token().symbol(&chain);

    rsx! {
        div {
            class: "page send-page page-enter-right {leave_anim}",
            ontouchstart: on_touch_start,
            ontouchend: on_touch_end,
            onanimationend: on_animation_end,
            div { class: "topbar",
                button {
                    class: "btn btn-small btn-secondary",
                    title: "返回",
                    onclick: move |_| go_back(),
                    "←"
                }
                span { class: "topbar-title", "📤 发送" }
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

            LoginSheet {
                visible: login_visible,
                leaving: login_leaving,
                password: login_password,
                error: login_error,
                loading: login_loading,
            }
        }
    }
}

// ============ 接收页面 ============

#[component]
fn ReceivePage(page: Signal<Page>, page_stack: Signal<Vec<Page>>) -> Element {
    let address = get_current_wallet_address();

    let mut copied = use_signal(|| false);

    let mut leave_anim = use_signal(|| String::new());
    let mut touch_start = use_signal(|| (0.0f64, 0.0f64));

    let go_back = move || {
        let mut stack = page_stack;
        let mut p = page;
        if let Some(prev) = stack.write().pop() {
            p.set(prev);
        }
    };

    let on_touch_start = move |evt: Event<TouchData>| {
        if let Some(touch) = evt.touches().first() {
            let coords = touch.client_coordinates();
            touch_start.set((coords.x, coords.y));
        }
    };

    let on_touch_end = move |evt: Event<TouchData>| {
        let (start_x, start_y) = touch_start();
        if let Some(touch) = evt.touches_changed().first() {
            let coords = touch.client_coordinates();
            let dx = coords.x - start_x;
            let dy = coords.y - start_y;
            if dx.abs() > 100.0 && dx.abs() > dy.abs() {
                if dx > 0.0 {
                    leave_anim.set("page-leave-right".into());
                } else {
                    leave_anim.set("page-leave-left".into());
                }
            }
        }
    };

    let on_animation_end = move |_: Event<AnimationData>| {
        if !leave_anim().is_empty() {
            go_back();
        }
    };

    rsx! {
        div {
            class: "page receive-page page-enter-right {leave_anim}",
            ontouchstart: on_touch_start,
            ontouchend: on_touch_end,
            onanimationend: on_animation_end,
            div { class: "topbar",
                button {
                    class: "btn btn-small btn-secondary",
                    title: "返回",
                    onclick: move |_| go_back(),
                    "←"
                }
                span { class: "topbar-title", "📥 接收" }
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
enum SwapToken {
    Native,
    USDC,
    USDT,
}

impl SwapToken {
    fn symbol(&self, chain: &ChainInfo) -> String {
        match self {
            SwapToken::Native => chain.native_symbol.to_string(),
            SwapToken::USDC => "USDC".to_string(),
            SwapToken::USDT => "USDT".to_string(),
        }
    }

    fn decimals(&self, chain: &ChainInfo) -> u8 {
        match self {
            SwapToken::Native => 18,
            SwapToken::USDC | SwapToken::USDT => {
                // BNB Smart Chain (chain_id: 56) 上的 USDC 和 USDT 使用 18 位精度
                if chain.chain_id == 56 { 18 } else { 6 }
            }
        }
    }

    /// 返回代币合约地址；原生代币返回 "native"
    fn address(&self, chain: &ChainInfo) -> Option<String> {
        match self {
            SwapToken::Native => Some("native".to_string()),
            SwapToken::USDC => chain.usdc_address.map(|s| s.to_string()),
            SwapToken::USDT => chain.usdt_address.map(|s| s.to_string()),
        }
    }

    /// 返回获取报价时使用的地址（原生代币用 WETH）
    fn quote_address(&self, chain: &ChainInfo) -> Option<String> {
        match self {
            SwapToken::Native => {
                blockchain::swap::weth_address(chain.chain_id).map(|s| s.to_string())
            }
            SwapToken::USDC => chain.usdc_address.map(|s| s.to_string()),
            SwapToken::USDT => chain.usdt_address.map(|s| s.to_string()),
        }
    }

    /// 转换为 TokenType（用于查询余额）
    fn to_token_type(&self) -> TokenType {
        match self {
            SwapToken::Native => TokenType::Native,
            SwapToken::USDC => TokenType::USDC,
            SwapToken::USDT => TokenType::USDT,
        }
    }
}

#[component]
fn SwapPage(page: Signal<Page>, page_stack: Signal<Vec<Page>>) -> Element {
    let chain = SELECTED_CHAIN.read().clone();
    let chain_supported = blockchain::swap::is_chain_supported(chain.chain_id);

    let address = get_current_wallet_address();

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

    let mut login_visible = use_signal(|| false);
    let mut login_password = use_signal(|| String::new());
    let mut login_error = use_signal(|| String::new());
    let login_loading = use_signal(|| false);
    let login_leaving = use_signal(|| false);

    let addr_for_max = address.clone();

    // 若有待执行的交换操作，恢复表单数据
    use_effect(move || {
        if let PendingOp::Swap {
            amount_str: ref amt,
            ..
        } = PENDING_OP.read().clone()
        {
            amount_str.set(amt.clone());
        }
    });

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
            let contract_address = token_type.contract_address(&chain);
            let balance_units = match blockchain::read::fetch_balance_units(
                &addr,
                chain.rpc_url,
                contract_address.as_deref(),
            )
            .await
            {
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
            let display_str =
                blockchain::read::format_units_to_decimal(balance_units, decimals, None);
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

        if from == to {
            error_msg.set("输入和输出代币不能相同".into());
            return;
        }

        if amount_input.is_empty() {
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

        let token_in_addr = match from.quote_address(&chain) {
            Some(addr) => addr,
            None => {
                error_msg.set("该链不支持此输入代币".into());
                return;
            }
        };

        let token_out_addr = match to.quote_address(&chain) {
            Some(addr) => addr,
            None => {
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
            match blockchain::swap::get_quote_with_rpc_url(
                chain.rpc_url,
                &token_in_addr,
                &token_out_addr,
                &amount_in_hex,
                fee,
            )
            .await
            {
                Ok(quote) => {
                    let formatted =
                        blockchain::swap::format_token_amount(quote.amount_out, to_decimals);
                    quote_result.set(format!("≈ {} {}", formatted, to_symbol));
                    quote_amount_out.set(Some(quote.amount_out));
                }
                Err(e) => {
                    error_msg.set(format!("获取报价失败: {}", e));
                }
            }
            quote_loading.set(false);
        });
    };

    // 执行交换
    let address_for_swap = address.clone();
    let on_swap = move |_| {
        let amount_input = amount_str().trim().to_string();
        let from = token_from();
        let to = token_to();
        let chain = SELECTED_CHAIN.read().clone();
        let fee = fee_tier();

        if from == to {
            error_msg.set("输入和输出代币不能相同".into());
            return;
        }

        if amount_input.is_empty() {
            error_msg.set("请输入金额".into());
            return;
        }

        // 根据代币类型解析金额（与 on_get_quote 保持一致，纯整数）
        let amount_in: u128 = match from {
            SwapToken::Native => match parse_token_amount_to_units(&amount_input, 18) {
                Ok(v) => v,
                Err(e) => {
                    error_msg.set(e);
                    return;
                }
            },
            SwapToken::USDC | SwapToken::USDT => {
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
        let slippage_times_10: u32 = match slippage_str().trim().parse::<f32>() {
            Ok(v) if v > 0.0 && v < 50.0 => (v * 10.0) as u32,
            _ => {
                error_msg.set("滑点必须在 0-50% 之间".into());
                return;
            }
        };

        let quoted_out = match quote_amount_out() {
            Some(v) => v,
            None => {
                error_msg.set("请先获取报价".into());
                return;
            }
        };

        // 计算最小输出（考虑滑点）- 使用纯整数运算
        let amount_out_min = quoted_out * (1000 - slippage_times_10 as u128) / 1000;

        let token_in_addr = match from.address(&chain) {
            Some(addr) => addr,
            None => {
                error_msg.set("该链不支持此输入代币".into());
                return;
            }
        };

        let token_out_addr = match to.address(&chain) {
            Some(addr) => addr,
            None => {
                error_msg.set("该链不支持此输出代币".into());
                return;
            }
        };

        // 检查私钥
        if UNLOCKED_SK.read().is_none() {
            *PENDING_OP.write() = PendingOp::Swap {
                token_in_addr,
                token_out_addr,
                amount_in,
                amount_str: amount_input,
                amount_out_min,
                fee,
                recipient: address_for_swap.clone(),
                chain_id: chain.chain_id,
                rpc_url: chain.rpc_url.to_string(),
            };
            login_password.set(String::new());
            login_error.set(String::new());
            login_visible.set(true);
            return;
        }

        let sk_hex = UNLOCKED_SK.read().clone().unwrap();
        let my_addr = address_for_swap.clone();

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
            match blockchain::swap::execute_swap_with_rpc_url(
                chain.rpc_url,
                &swap_params,
                &sk_hex,
            )
            .await
            {
                Ok(tx_hash) => {
                    success_msg.set(format!("交换成功！交易哈希: {}", tx_hash));
                    amount_str.set(String::new());
                    quote_result.set(String::new());
                    quote_amount_out.set(None);
                    *PENDING_OP.write() = PendingOp::None;
                }
                Err(e) => {
                    error_msg.set(format!("交换失败: {}", e));
                }
            }
            swapping.set(false);
            if let Some(mut sk) = UNLOCKED_SK.write().take() {
                sk.zeroize();
            }
        });
    };

    // 当解锁完成后自动执行暂存的交换操作
    use_effect(move || {
        let op = PENDING_OP.read().clone();
        match op {
            PendingOp::Swap {
                token_in_addr,
                token_out_addr,
                amount_in,
                amount_out_min,
                fee,
                recipient,
                chain_id,
                rpc_url,
                ..
            } => {
                if let Some(sk_hex) = UNLOCKED_SK.read().clone() {
                    swapping.set(true);
                    error_msg.set(String::new());
                    success_msg.set(String::new());
                    let swap_params = blockchain::swap::SwapParams {
                        token_in: token_in_addr,
                        token_out: token_out_addr,
                        amount_in,
                        amount_out_min,
                        fee,
                        recipient,
                        chain_id,
                    };
                    spawn(async move {
                        match blockchain::swap::execute_swap_with_rpc_url(
                            &rpc_url,
                            &swap_params,
                            &sk_hex,
                        )
                        .await
                        {
                            Ok(tx_hash) => {
                                success_msg.set(format!("交换成功！交易哈希: {}", tx_hash));
                                amount_str.set(String::new());
                                quote_result.set(String::new());
                                quote_amount_out.set(None);
                                *PENDING_OP.write() = PendingOp::None;
                            }
                            Err(e) => {
                                error_msg.set(format!("交换失败: {}", e));
                            }
                        }
                        swapping.set(false);
                        if let Some(mut sk) = UNLOCKED_SK.write().take() {
                            sk.zeroize();
                        }
                    });
                }
            }
            _ => {}
        }
    });

    let mut leave_anim = use_signal(|| String::new());
    let mut touch_start = use_signal(|| (0.0f64, 0.0f64));

    let go_back = move || {
        *PENDING_OP.write() = PendingOp::None;
        let mut stack = page_stack;
        let mut p = page;
        if let Some(prev) = stack.write().pop() {
            p.set(prev);
        }
    };

    let on_touch_start = move |evt: Event<TouchData>| {
        if let Some(touch) = evt.touches().first() {
            let coords = touch.client_coordinates();
            touch_start.set((coords.x, coords.y));
        }
    };

    let on_touch_end = move |evt: Event<TouchData>| {
        let (start_x, start_y) = touch_start();
        if let Some(touch) = evt.touches_changed().first() {
            let coords = touch.client_coordinates();
            let dx = coords.x - start_x;
            let dy = coords.y - start_y;
            if dx.abs() > 100.0 && dx.abs() > dy.abs() {
                if dx > 0.0 {
                    leave_anim.set("page-leave-right".into());
                } else {
                    leave_anim.set("page-leave-left".into());
                }
            }
        }
    };

    let on_animation_end = move |_: Event<AnimationData>| {
        if !leave_anim().is_empty() {
            go_back();
        }
    };

    let native_symbol = chain.native_symbol.to_string();
    let native_icon = chain.native_icon;
    let has_usdt = chain.usdt_address.is_some();
    let from_symbol = token_from().symbol(&chain);

    rsx! {
        div {
            class: "page swap-page page-enter-right {leave_anim}",
            ontouchstart: on_touch_start,
            ontouchend: on_touch_end,
            onanimationend: on_animation_end,
            div { class: "topbar",
                button {
                    class: "btn btn-small btn-secondary",
                    title: "返回",
                    onclick: move |_| go_back(),
                    "←"
                }
                span { class: "topbar-title", "🔄 交换" }
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

            LoginSheet {
                visible: login_visible,
                leaving: login_leaving,
                password: login_password,
                error: login_error,
                loading: login_loading,
            }
        }
    }
}

// ============ 钱包详情模态窗口 ============

#[component]
fn WalletDetailModal(wallet: WalletInfo, on_close: EventHandler<()>) -> Element {
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
                            span { class: "wallet-detail-value", style: "color: #999;", "（重新解锁钱包后自动填充）" }
                        }
                    }

                    // 创建时间
                    div { class: "wallet-detail-row",
                        span { class: "wallet-detail-label", "创建时间" }
                        span { class: "wallet-detail-value wallet-detail-mono", "{created_time}" }
                    }

                    // 钱包 ID
                    div { class: "wallet-detail-row",
                        span { class: "wallet-detail-label", "钱包 ID" }
                        span { class: "wallet-detail-value wallet-detail-mono", "{wallet.id}" }
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
fn TxItem(
    tx: TxRecord,
    my_address: String,
    page: Signal<Page>,
    page_stack: Signal<Vec<Page>>,
) -> Element {
    let is_out = tx.from.to_lowercase() == my_address.to_lowercase();
    let dir = if is_out { "↑" } else { "↓" };
    let status = if tx.is_error { "❌" } else { "✅" };
    let cp = if is_out {
        shorten_address(&tx.to)
    } else {
        shorten_address(&tx.from)
    };
    let native_symbol = data::all_chains()
        .iter()
        .find(|c| c.chain_id == tx.chain_id)
        .map(|c| c.native_symbol.to_string())
        .unwrap_or_default();
    let value_text = if tx.token_type == "Native" {
        format!("{} {}", tx.value_display, native_symbol)
    } else {
        format!("{} {}", tx.token_amount, tx.token_symbol)
    };
    let tx_for_detail = tx.clone();

    rsx! {
        div {
            class: "asset-tx-item",
            onclick: move |_| {
                let mut stack = page_stack;
                let mut p = page;
                stack.write().push(p());
                p.set(Page::TransactionDetail(tx_for_detail.clone()));
            },
            div { class: "asset-tx-left",
                span { class: "asset-tx-dir", "{dir}" }
                span { class: "asset-tx-addr", "{cp}" }
            }
            div { class: "asset-tx-right",
                span { class: "asset-tx-value", "{value_text}" }
                span { class: "asset-tx-status", "{status}" }
            }
            div { class: "asset-tx-time", "{tx.timestamp}" }
        }
    }
}

// ============ 资产详情页面 ============

#[component]
fn AssetDetailPage(
    page: Signal<Page>,
    page_stack: Signal<Vec<Page>>,
    chain_id: u64,
    chain_name: String,
    token: TokenType,
    symbol: String,
    enter_anim: bool,
) -> Element {
    let mut balance_str = use_signal(|| "加载中...".to_string());
    let mut transactions: Signal<Vec<TxRecord>> = use_signal(Vec::new);
    let mut tx_loading = use_signal(|| false);
    let mut tx_error = use_signal(|| String::new());
    let mut leave_anim = use_signal(|| String::new());
    let mut touch_start = use_signal(|| (0.0f64, 0.0f64));
    let mut refresh_counter = use_signal(|| 0u32);

    let go_back = move || {
        let mut stack = page_stack;
        let mut p = page;
        if let Some(prev) = stack.write().pop() {
            p.set(prev);
        }
    };

    // 获取精确余额
    {
        let token = token.clone();
        let chain_id_copy = chain_id;
        let _chain_name = chain_name.clone();
        use_effect(move || {
            let _ = refresh_counter();
            let cache_key = balance_key(chain_id_copy, &token);
            if let Some(cached) = BALANCE_CACHE.read().get(&cache_key) {
                match cached {
                    Ok(bal) => balance_str.set(bal.clone()),
                    Err(e) => balance_str.set(format!("错误: {}", e)),
                }
                return;
            }
            let chains = all_chains();
            if let Some(chain) = chains.iter().find(|c| c.chain_id == chain_id_copy) {
                let addr = get_current_wallet_address();
                let t = token.clone();
                let c = chain.clone();
                balance_str.set("加载中...".to_string());
                if let Some(wallet_id) = CURRENT_WALLET_ID.read().clone() {
                    data::enqueue_balance(&wallet_id, &addr, c, t, RequestPriority::High);
                }
            }
        });
    }

    // 获取交易记录
    {
        let chain_id_copy = chain_id;
        let token_copy = token.clone();
        use_effect(move || {
            let _ = *TX_SERIAL.read();
            let _ = refresh_counter();
            let chain_id = chain_id_copy;
            let cache_key = tx_cache_key(chain_id, &token_copy);

            if let Some(cached) = TX_CACHE.read().get(&cache_key) {
                match cached {
                    Ok(txs) => {
                        tx_error.set(String::new());
                        transactions.set(txs.clone());
                        tx_loading.set(false);
                    }
                    Err(e) => {
                        transactions.set(Vec::new());
                        tx_error.set(format!("获取交易历史失败: {}", e));
                        tx_loading.set(false);
                    }
                }
                return;
            }

            let chains = all_chains();
            if let Some(chain) = chains.iter().find(|c| c.chain_id == chain_id) {
                let addr = get_current_wallet_address();
                let c = chain.clone();
                transactions.set(Vec::new());
                tx_loading.set(true);
                tx_error.set(String::new());
                if let Some(wallet_id) = CURRENT_WALLET_ID.read().clone() {
                    data::enqueue_transactions(&wallet_id, &addr, c, RequestPriority::High);
                }
            }
        });
    }

    let on_touch_start = move |evt: Event<TouchData>| {
        if let Some(touch) = evt.touches().first() {
            let coords = touch.client_coordinates();
            touch_start.set((coords.x, coords.y));
        }
    };

    let on_touch_end = move |evt: Event<TouchData>| {
        let (start_x, start_y) = touch_start();
        if let Some(touch) = evt.touches_changed().first() {
            let coords = touch.client_coordinates();
            let dx = coords.x - start_x;
            let dy = coords.y - start_y;
            if dx.abs() > 100.0 && dx.abs() > dy.abs() {
                if dx > 0.0 {
                    leave_anim.set("page-leave-right".into());
                } else {
                    leave_anim.set("page-leave-left".into());
                }
            }
        }
    };

    let on_animation_end = move |_: Event<AnimationData>| {
        if !leave_anim().is_empty() {
            go_back();
        }
    };

    let title = format!("{} ({})", symbol, chain_name);
    let enter_anim_class = if enter_anim { "page-enter-right" } else { "" };

    rsx! {
        div {
            class: "page {enter_anim_class} {leave_anim}",
            ontouchstart: on_touch_start,
            ontouchend: on_touch_end,
            onanimationend: on_animation_end,
            div { class: "topbar",
                button {
                    class: "btn btn-small btn-secondary",
                    title: "返回",
                    onclick: move |_| go_back(),
                    "←"
                }
                span { class: "topbar-title", "{title}" }
            }

            div { class: "card", style: "text-align: center;",
                h2 { class: "section-title", style: "text-align: center;", "余额" }
                p { class: "asset-detail-balance", "{balance_str} {symbol}" }
            }

            div { class: "card",
                div { class: "tx-header",
                    h2 { class: "section-title", "交易记录" }
                    button {
                        class: "btn btn-small btn-secondary",
                        onclick: move |_| {
                            if let Some(ref wid) = *CURRENT_WALLET_ID.read() {
                                data::clear_balance(wid, chain_id, &token);
                                data::clear_transaction_group(wid, chain_id, &token);
                            }
                            refresh_counter.set(refresh_counter() + 1);
                        },
                        "刷新"
                    }
                }

                if tx_loading() {
                    p { class: "loading-text", "加载交易历史中..." }
                }
                if !tx_error().is_empty() {
                    p { class: "error", "{tx_error}" }
                }
                if transactions().is_empty() && !tx_loading() && tx_error().is_empty() {
                    p { class: "empty-text", "暂无交易记录" }
                }

                for tx_rec in transactions() {
                    {
                        let tx_clone = tx_rec.clone();
                        let my_addr = get_current_wallet_address();
                        let is_out = tx_clone.from.to_lowercase() == my_addr.to_lowercase();
                        let dir = if is_out { "↑" } else { "↓" };
                        let status = if tx_clone.is_error { "❌" } else { "✅" };
                        let cp = if is_out { shorten_address(&tx_clone.to) } else { shorten_address(&tx_clone.from) };
                        let value_text = if tx_clone.token_type == "Native" {
                            format!("{} {}", tx_clone.value_display, symbol)
                        } else {
                            format!("{} {}", tx_clone.token_amount, tx_clone.token_symbol)
                        };
                        rsx! {
                            div {
                                class: "asset-tx-item",
                                onclick: {
                                    let tx_c = tx_clone.clone();
                                    let mut stack = page_stack;
                                    let mut p = page;
                                    let push_page = Page::AssetDetail {
                                        chain_id, chain_name: chain_name.clone(),
                                        token: token.clone(), symbol: symbol.clone(),
                                        enter_anim: false,
                                    };
                                    move |_| {
                                        stack.write().push(push_page.clone());
                                        p.set(Page::TransactionDetail(tx_c.clone()));
                                    }
                                },
                                div { class: "asset-tx-left",
                                    span { class: "asset-tx-dir", "{dir}" }
                                    span { class: "asset-tx-addr", "{cp}" }
                                }
                                div { class: "asset-tx-right",
                                    span { class: "asset-tx-value", "{value_text}" }
                                    span { class: "asset-tx-status", "{status}" }
                                }
                                div { class: "asset-tx-time", "{tx_rec.timestamp}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ============ 交易详情页面 ============

#[component]
fn TransactionDetailPage(
    page: Signal<Page>,
    page_stack: Signal<Vec<Page>>,
    tx: TxRecord,
) -> Element {
    let chain_id = tx.chain_id;
    let chain = data::all_chains()
        .into_iter()
        .find(|c| c.chain_id == chain_id)
        .unwrap_or_else(data::default_chain);
    let explorer_tx_url = format!("{}/tx/{}", chain.explorer_url, tx.hash);
    let (status_icon, status_text) = if tx.is_error {
        ("❌", "失败")
    } else {
        ("✅", "成功")
    };
    let (direction_icon, direction_text) = if tx.is_outgoing {
        ("↑", "发送")
    } else {
        ("↓", "接收")
    };
    let explorer_url_clone = explorer_tx_url.clone();

    let mut leave_anim = use_signal(|| String::new());
    let mut touch_start = use_signal(|| (0.0f64, 0.0f64));

    let go_back = move || {
        let mut stack = page_stack;
        let mut p = page;
        if let Some(prev) = stack.write().pop() {
            p.set(prev);
        }
    };

    let on_touch_start = move |evt: Event<TouchData>| {
        if let Some(touch) = evt.touches().first() {
            let coords = touch.client_coordinates();
            touch_start.set((coords.x, coords.y));
        }
    };

    let on_touch_end = move |evt: Event<TouchData>| {
        let (start_x, start_y) = touch_start();
        if let Some(touch) = evt.touches_changed().first() {
            let coords = touch.client_coordinates();
            let dx = coords.x - start_x;
            let dy = coords.y - start_y;
            if dx.abs() > 100.0 && dx.abs() > dy.abs() {
                if dx > 0.0 {
                    leave_anim.set("page-leave-right".into());
                } else {
                    leave_anim.set("page-leave-left".into());
                }
            }
        }
    };

    let on_animation_end = move |_: Event<AnimationData>| {
        if !leave_anim().is_empty() {
            go_back();
        }
    };

    rsx! {
        div {
            class: "page page-enter-right {leave_anim}",
            ontouchstart: on_touch_start,
            ontouchend: on_touch_end,
            onanimationend: on_animation_end,
            div { class: "topbar",
                button {
                    class: "btn btn-small btn-secondary",
                    title: "返回",
                    onclick: move |_| go_back(),
                    "←"
                }
                span { class: "topbar-title", "📋 交易详情" }
            }

            div { class: "card",
                div { class: "tx-detail-list",
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "状态" }
                        span { class: "tx-detail-value",
                            span { class: "tx-detail-leading-symbol", "{status_icon}" }
                            "{status_text}"
                        }
                    }
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "类型" }
                        span { class: "tx-detail-value",
                            span { class: "tx-detail-leading-symbol", "{direction_icon}" }
                            "{direction_text}"
                        }
                    }
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "金额" }
                        span { class: "tx-detail-value",
                            {
                                if tx.token_type == "Native" {
                                    format!("{} {}", tx.value_display, chain.native_symbol)
                                } else {
                                    format!("{} {}", tx.token_amount, tx.token_symbol)
                                }
                            }
                        }
                    }
                    if tx.token_type == "Native" {
                        div { class: "tx-detail-row",
                            span { class: "tx-detail-label", "Wei" }
                            span { class: "tx-detail-value tx-detail-mono", "{tx.value_wei}" }
                        }
                    } else {
                        div { class: "tx-detail-row",
                            span { class: "tx-detail-label", "合约地址" }
                            span { class: "tx-detail-value tx-detail-mono tx-detail-address", "{tx.token_contract}" }
                        }
                    }
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "发送方" }
                        span { class: "tx-detail-value tx-detail-mono tx-detail-address", "{tx.from}" }
                    }
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "接收方" }
                        span { class: "tx-detail-value tx-detail-mono tx-detail-address", "{tx.to}" }
                    }
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "交易哈希" }
                        span { class: "tx-detail-value tx-detail-mono tx-detail-address", "{tx.hash}" }
                    }
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "区块号" }
                        span { class: "tx-detail-value", "{tx.block_number}" }
                    }
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "Gas 用量" }
                        span { class: "tx-detail-value", "{tx.gas_used}" }
                    }
                    div { class: "tx-detail-row",
                        span { class: "tx-detail-label", "时间" }
                        span { class: "tx-detail-value", "{tx.timestamp}" }
                    }
                }
                button {
                    class: "btn btn-primary",
                    style: "margin-top: 16px;",
                    r#type: "button",
                    onclick: move |_| {},
                    "onmousedown": format!("window.open('{}', '_system');", explorer_url_clone),
                    "🔗 在区块浏览器中查看"
                }
            }
        }
    }
}

// ============ 全局状态 ============

static UNLOCKED_SK: dioxus::prelude::GlobalSignal<Option<String>> = GlobalSignal::new(|| None);
static CURRENT_WALLET_ID: dioxus::prelude::GlobalSignal<Option<String>> =
    GlobalSignal::new(|| None);
static SELECTED_CHAIN: dioxus::prelude::GlobalSignal<ChainInfo> =
    GlobalSignal::new(|| default_chain());

/// 待执行的操作（签名后自动执行）
static PENDING_OP: dioxus::prelude::GlobalSignal<PendingOp> = GlobalSignal::new(|| PendingOp::None);

// ============ 辅助函数 ============

// ============ 钱包列表管理 ============

/// 获取数据根目录（Android files 目录）
fn get_data_dir() -> Option<std::path::PathBuf> {
    data::app_files_dir()
}

/// 获取钱包 keystore 文件的完整路径: <exe_dir>/wallets/<id>
fn get_wallet_keystore_path(wallet_id: &str) -> std::path::PathBuf {
    let dir = get_data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    dir.join(WALLETS_DIR).join(format!("{}", wallet_id))
}

/// 获取 wallet_list.json 的完整路径（放在 wallets 文件夹中）
fn get_wallet_list_path() -> std::path::PathBuf {
    let dir = get_data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let wallets_dir = dir.join(WALLETS_DIR);
    // 确保 wallets 目录存在
    let _ = std::fs::create_dir_all(&wallets_dir);
    wallets_dir.join(WALLET_LIST_FILE)
}

/// 读取钱包列表；文件不存在则返回空列表
fn load_wallet_list() -> WalletList {
    let path = get_wallet_list_path();
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(list) = serde_json::from_str::<WalletList>(&data) {
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
fn save_wallet_list(list: &WalletList) {
    let path = get_wallet_list_path();
    if let Ok(data) = serde_json::to_string_pretty(list) {
        let _ = std::fs::write(path, data);
    }
}

fn delete_wallet(wallet_id: &str, list: &mut WalletList) {
    let _ = std::fs::remove_file(get_wallet_keystore_path(wallet_id));
    data::remove_wallet_cache(wallet_id);
    list.wallets.retain(|wallet| wallet.id != wallet_id);
    if list.default_wallet_id.as_deref() == Some(wallet_id) {
        list.default_wallet_id = list.wallets.first().map(|wallet| wallet.id.clone());
    }
    save_wallet_list(list);
}

/// 生成唯一钱包 ID（基于公钥 x 坐标的前 8 字节）
///
/// # 参数
/// - `private_key_hex`: 私钥的十六进制字符串
///
/// # 返回
/// - 钱包 ID（16 位十六进制字符，即公钥 x 坐标的前 8 字节）
fn generate_wallet_id(private_key_hex: &str) -> Result<String, String> {
    // 获取公钥坐标
    let (x_hex, _y_hex) = blockchain::account::private_key_hex_to_public_key_xy(private_key_hex)?;

    // 取 x 坐标的前 8 字节（16 个十六进制字符）
    let id = x_hex.chars().take(16).collect::<String>();

    Ok(id)
}

/// 获取当前 Unix 时间戳（秒）
fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// 获取当前钱包的地址（从钱包列表读取，不依赖私钥）
fn get_current_wallet_address() -> String {
    if let Some(ref wid) = *CURRENT_WALLET_ID.read() {
        let list = load_wallet_list();
        if let Some(w) = list.wallets.iter().find(|w| w.id == *wid) {
            return w.address.clone();
        }
    }
    "未找到钱包".to_string()
}

fn shorten_address(addr: &str) -> String {
    if addr.len() > 12 {
        format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
    } else {
        addr.to_string()
    }
}

fn format_timestamp(unix_str: &str) -> String {
    if let Ok(ts) = unix_str.parse::<i64>() {
        // 转换为 UTC+8 时间（东八区）
        let utc8_offset = 8 * 3600; // 8小时的秒数
        let secs = ts + utc8_offset;
        let days_since_epoch = secs / 86400;
        let time_of_day = secs % 86400;
        let hours = time_of_day / 3600;
        let minutes = (time_of_day % 3600) / 60;

        let (year, month, day) = days_to_ymd(days_since_epoch);
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02} (UTC+8)",
            year, month, day, hours, minutes
        )
    } else {
        unix_str.to_string()
    }
}

fn days_to_ymd(days: i64) -> (i64, i64, i64) {
    let mut y = 1970;
    let mut remaining = days;

    loop {
        let days_in_year = if is_leap_year(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }

    let months_days = if is_leap_year(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 1;
    for &md in &months_days {
        if remaining < md {
            break;
        }
        remaining -= md;
        m += 1;
    }

    (y, m, remaining + 1)
}

fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// 将全精度余额字符串截断到 8 位小数（仅用于主页展示）
fn truncate_balance_for_display(bal: &str) -> String {
    if let Some(dot_pos) = bal.find('.') {
        let int_part = &bal[..dot_pos];
        let frac_part = &bal[dot_pos + 1..];
        if frac_part.len() > 8 {
            return format!("{}.{}", int_part, &frac_part[..8]);
        }
    }
    bal.to_string()
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
        let integer_part: u128 = parts[0].parse().map_err(|_| "金额格式无效".to_string())?;

        // 处理小数部分
        let mut decimal_part = parts[1].to_string();
        if decimal_part.len() > decimals as usize {
            return Err(format!("最多支持 {} 位小数", decimals));
        }

        // 补齐到指定精度
        while decimal_part.len() < decimals as usize {
            decimal_part.push('0');
        }

        let decimal_value: u128 = decimal_part
            .parse()
            .map_err(|_| "金额格式无效".to_string())?;

        // 计算最小单位：整数部分 * 10^decimals + 小数部分
        let divisor = 10u128.pow(decimals as u32);
        Ok(integer_part * divisor + decimal_value)
    } else {
        // 没有小数点，直接解析整数部分
        let integer_part: u128 = amount_str.parse().map_err(|_| "金额格式无效".to_string())?;
        let divisor = 10u128.pow(decimals as u32);
        Ok(integer_part * divisor)
    }
}

// ============ CSS 样式 ============

const CSS: &str = r#"
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
    -webkit-tap-highlight-color: transparent;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
    background: #f5f5f5;
    color: #333;
    min-height: 100vh;
}

.app-container {
    width: 100%;
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    overflow: hidden;
}

.page {
    width: 100%;
    max-width: 100%;
    margin: 0 auto;
    padding: 16px;
    padding-top: 36px;
    min-height: 100vh;
}

.import-page {
    display: flex;
    align-items: center;
    justify-content: center;
}

.login-overlay {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    left: 0;
    background: rgba(0, 0, 0, 0.65);
    display: flex;
    align-items: flex-end;
    justify-content: center;
    z-index: 1000;
    padding: 16px;
    padding-bottom: max(16px, env(safe-area-inset-bottom));
}

.login-page {
    position: fixed;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 20;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    min-height: auto;
    padding: 16px;
    padding-bottom: max(16px, env(safe-area-inset-bottom));
}

.login-sheet {
    width: min(100%, 480px);
    max-height: min(72vh, 560px);
    margin-bottom: 0;
    overflow-y: auto;
    animation: login-sheet-up 0.28s ease-out;
}

.login-sheet.leaving {
    animation: login-sheet-down 0.22s ease-in forwards;
}

.wallet-page {
    padding-top: 16px;
    padding-bottom: 70px;
}

.card {
    background: #ffffff;
    border-radius: 16px;
    padding: 20px;
    margin-bottom: 16px;
    border: 1px solid #e0e0e0;
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
    color: #999;
    margin-bottom: 24px;
    font-size: 14px;
}

.form-group {
    margin-bottom: 16px;
}

.form-group label {
    display: block;
    font-size: 13px;
    color: #999;
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
    background: #ffffff;
    border: 1px solid #ddd;
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
    background: #f5f5f5;
    border: 1px solid #ddd;
    border-radius: 10px;
    color: #333;
    font-size: 14px;
    outline: none;
    transition: border-color 0.2s;
}

.form-group input:focus {
    border-color: #667eea;
}

.form-group input::placeholder {
    color: #bbb;
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
    background: #e0e0e0;
    margin: 16px 0;
}

.topbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
    padding: 8px 0;
    position: relative;
}

.topbar-title {
    font-size: 18px;
    font-weight: 700;
    background: linear-gradient(135deg, #667eea, #764ba2);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
}

.wallet-list-page .topbar-title,
.wallet-page .topbar-title {
    position: static;
    transform: none;
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
    border: 1px solid #e0e0e0;
    background: #ffffff;
    color: #999;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
}

.token-tab:hover {
    border-color: #667eea;
    color: #555;
}

.token-tab.active {
    border-color: #667eea;
    background: rgba(102, 126, 234, 0.12);
    color: #222;
}

.token-tab-icon {
    font-size: 16px;
    display: inline-block;
    min-width: 20px;
    text-align: center;
}

.account-card {
    text-align: center;
}

.account-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
}

.account-header .section-title {
    margin-bottom: 0;
}

.account-address {
    margin-bottom: 16px;
}

.account-address .label {
    display: block;
    font-size: 12px;
    color: #999;
    margin-bottom: 4px;
}

.account-address .address {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 14px;
    color: #999;
    cursor: default;
}

.account-balance {
    margin-bottom: 20px;
}

.balance-value {
    font-size: 36px;
    font-weight: 700;
    color: #222;
    margin-right: 8px;
}

.balance-unit {
    font-size: 18px;
    color: #999;
}

.action-buttons {
    display: flex;
    gap: 8px;
}

.tx-card {
    max-height: 400px;
    overflow-y: auto;
}

.tx-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
}

.tx-header .section-title {
    margin-bottom: 0;
}

.section-title {
    font-size: 16px;
    font-weight: 600;
    margin-bottom: 12px;
    color: #555;
}

.loading-text, .empty-text {
    color: #999;
    text-align: center;
    padding: 20px;
    font-size: 14px;
}

.tx-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

/* ======== 发送页面 ======== */

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
    color: #999;
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
    background: #f5f5f5;
    border: 1px solid #ddd;
    border-radius: 10px;
    padding: 18px 16px;
    margin: 16px 0 0 0;
    text-align: center;
    word-break: break-all;
}

.full-address {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 15px;
    color: #333;
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
    border: 1px solid #e0e0e0;
    background: #ffffff;
    color: #555;
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
    display: inline-block;
    min-width: 24px;
    text-align: center;
}

.chain-selector-name {
    flex: 1;
    text-align: left;
}

.chain-selector-arrow {
    color: #667eea;
    font-size: 12px;
}

.chain-selector-container {
    position: relative;
    margin-bottom: 10px;
}

.chain-dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    z-index: 100;
    background: #ffffff;
    border: 1px solid #e0e0e0;
    border-top: none;
    border-radius: 0 0 12px 12px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.1);
    max-height: 360px;
    overflow-y: auto;
    animation: dropdown-expand 0.3s ease-out;
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
    position: relative;
    background: #ffffff;
    border: 1px solid #e0e0e0;
    border-radius: 16px;
    padding: 20px;
    width: 95%;
    max-width: 100%;
    max-height: 80vh;
    overflow-y: auto;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.15);
}

.modal-close-btn {
    position: absolute;
    top: 14px;
    right: 14px;
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #fff;
    border: 1.5px solid #d0d0d0;
    border-radius: 8px;
    color: #888;
    font-size: 16px;
    font-weight: 700;
    cursor: pointer;
    transition: all 0.2s;
    line-height: 1;
    padding: 0;
}

.modal-close-btn:hover {
    background: #f5f5f5;
    border-color: #999;
    color: #333;
}

.modal-title {
    font-size: 20px;
    font-weight: 700;
    text-align: center;
    margin-bottom: 4px;
    color: #333;
}

.modal-subtitle {
    text-align: center;
    color: #999;
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
    background: rgba(0, 0, 0, 0.02);
    color: #555;
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
    color: #333;
}

.chain-item-detail {
    font-size: 11px;
    color: #999;
    margin-top: 2px;
}

.chain-item-check {
    color: #667eea;
    font-weight: 700;
    font-size: 16px;
    flex-shrink: 0;
}

/* ======== 交易详情 ======== */

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
    border-bottom: 1px solid #ddd;
}

.tx-detail-row:last-child {
    border-bottom: none;
}

.tx-detail-label {
    font-size: 11px;
    color: #999;
    margin-bottom: 4px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.5px;
}

.tx-detail-value {
    font-size: 14px;
    color: #333;
    line-height: 1.4;
}

.tx-detail-leading-symbol {
    display: inline-block;
    width: 1.6em;
    text-align: center;
    color: #667eea;
}

.tx-detail-mono {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 12px;
}

.tx-detail-address {
    word-break: break-all;
    color: #555;
}

/* ======== 钱包列表页面 ======== */

.wallet-list-page {
    display: flex;
    flex-direction: column;
    padding-top: 16px;
}

.wallet-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
}

.wallet-list-item {
    border: 1px solid #e0e0e0;
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.02);
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
    color: #333;
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
    color: #999;
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
    background: #f5f5f5;
    border: 1px solid #667eea;
    border-radius: 6px;
    color: #333;
    font-size: 13px;
    outline: none;
}

.wallet-rename-input::placeholder {
    color: #bbb;
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
    border: 1px solid #e0e0e0;
    background: #ffffff;
    color: #555;
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
    background: #ffffff;
    border: 1px solid #e0e0e0;
    border-top: none;
    border-radius: 0 0 12px 12px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.1);
    max-height: 360px;
    overflow-y: auto;
    animation: dropdown-expand 0.3s ease-out;
}

.wallet-dropdown-item {
    border-bottom: 1px solid rgba(0, 0, 0, 0.04);
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
    color: #333;
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
    color: #999;
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
    border-top: 1px solid #e0e0e0;
    display: flex;
    justify-content: center;
    gap: 8px;
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
    border-bottom: 1px solid #ddd;
}

.wallet-detail-row:last-child {
    border-bottom: none;
}

.wallet-detail-label {
    font-size: 11px;
    color: #999;
    margin-bottom: 4px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.5px;
}

.wallet-detail-value {
    font-size: 14px;
    color: #333;
    line-height: 1.4;
}

.wallet-detail-mono {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 12px;
    word-break: break-all;
    color: #999;
}

/* ---- 交换页面 ---- */

.quote-display {
    margin-top: 12px;
    padding: 12px 16px;
    background: #f0f4ff;
    border-radius: 10px;
    border: 1px solid #e0e0e0;
    text-align: center;
}

.quote-label {
    font-size: 13px;
    color: #999;
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
    border: 1px solid #e0e0e0;
    background: transparent;
    color: #999;
    font-size: 12px;
    cursor: pointer;
    transition: all 0.2s;
    text-align: center;
}

.fee-tab:hover {
    border-color: #667eea;
    color: #555;
}

.fee-tab.active {
    background: #667eea33;
    border-color: #667eea;
    color: #667eea;
    font-weight: 600;
}

/* ======== 页面切换动画 ======== */

.page-enter-right {
    animation: slide-in-right 0.3s ease-out;
}

.page-leave-left {
    animation: slide-out-left 0.3s ease-in forwards;
}

.page-leave-right {
    animation: slide-out-right 0.3s ease-in forwards;
}

@keyframes slide-in-right {
    from { transform: translateX(100%); }
    to   { transform: translateX(0); }
}

@keyframes slide-out-left {
    from { transform: translateX(0); }
    to   { transform: translateX(-100%); }
}

@keyframes slide-out-right {
    from { transform: translateX(0); }
    to   { transform: translateX(100%); }
}

@keyframes login-sheet-up {
    from {
        opacity: 0;
        transform: translateY(100%);
    }
    to {
        opacity: 1;
        transform: translateY(0);
    }
}

@keyframes login-sheet-down {
    from {
        opacity: 1;
        transform: translateY(0);
    }
    to {
        opacity: 0;
        transform: translateY(100%);
    }
}

@keyframes dropdown-expand {
    from {
        opacity: 0;
        transform: translateY(-8px);
    }
    to {
        opacity: 1;
        transform: translateY(0);
    }
}

/* ======== 底部导航栏 ======== */

.bottom-bar {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    display: flex;
    justify-content: space-around;
    background: #fff;
    border-top: 1px solid #e0e0e0;
    z-index: 500;
    height: 50px;
}

.bottom-bar-btn {
    border: none;
    background: transparent;
    color: #999;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: color 0.2s;
    padding: 0 8px;
}

.bottom-bar-btn.active {
    color: #667eea;
}

.bottom-bar-btn:hover {
    color: #667eea;
}

/* ======== 资产页面 ======== */

.assets-page {
    padding-bottom: 60px;
}

.assets-ring-container {
    position: relative;
    width: 100%;
    max-width: 360px;
    margin: 0 auto 12px auto;
    display: flex;
    flex-direction: column;
    align-items: center;
}

.assets-ring-header {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    margin-bottom: 4px;
    width: 100%;
}

.ring-legend {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 8px;
}

.ring-legend-item {
    display: flex;
    align-items: center;
    gap: 8px;
}

.ring-legend-color {
    width: 14px;
    height: 10px;
    border-radius: 2px;
    flex-shrink: 0;
}

.ring-legend-label {
    font-size: 13px;
    color: #333;
}

.assets-toggle-row {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    margin-bottom: 12px;
    padding: 0 8px;
}

.toggle-zero-btn {
    width: 20px;
    height: 20px;
    border: 1.5px solid #ccc;
    border-radius: 4px;
    background: #fff;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition: all 0.2s;
    flex-shrink: 0;
}

.toggle-zero-btn.checked {
    background: #6bff6b;
    border-color: #6bff6b;
}

.toggle-zero-check {
    font-size: 14px;
    font-weight: 700;
    color: #fff;
    line-height: 1;
}

.toggle-zero-label {
    font-size: 13px;
    color: #666;
    cursor: pointer;
    user-select: none;
}

.asset-list {
    display: flex;
    flex-direction: column;
}

.asset-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 14px 16px;
    border-bottom: 1px solid #ddd;
    cursor: pointer;
}

.asset-item:last-child {
    border-bottom: none;
}

.asset-item-left {
    flex: 1;
    min-width: 0;
}

.asset-item-symbol {
    font-size: 14px;
    font-weight: 600;
    color: #333;
}

.asset-item-right {
    flex-shrink: 0;
    max-width: 55%;
}

.asset-item-balance {
    font-size: 13px;
    color: #333;
    font-family: "SF Mono", "Fira Code", monospace;
    word-break: break-all;
    text-align: right;
    display: block;
}

.asset-item-balance.zero {
    color: #bbb;
}

/* ======== 资产加载动画 ======== */

.assets-loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 40px 0;
    gap: 12px;
}

.assets-spinner {
    width: 36px;
    height: 36px;
    border: 3px solid #e0e0e0;
    border-top-color: #667eea;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
}

@keyframes spin {
    to { transform: rotate(360deg); }
}

/* ======== 资产详情页 ======== */

.asset-detail-balance {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 14px;
    color: #333;
    word-break: break-all;
    line-height: 1.6;
    text-align: center;
}

.asset-tx-item {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    padding: 12px 0;
    border-bottom: 1px solid #eee;
    cursor: pointer;
    gap: 4px;
}

.asset-tx-item:last-child {
    border-bottom: none;
}

.asset-tx-left {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    flex: 1;
}

.asset-tx-dir {
    font-size: 14px;
    font-weight: 700;
    color: #667eea;
    flex-shrink: 0;
}

.asset-tx-addr {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 12px;
    color: #555;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.asset-tx-right {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
}

.asset-tx-value {
    font-size: 13px;
    font-weight: 600;
    color: #333;
    font-family: "SF Mono", "Fira Code", monospace;
}

.asset-tx-status {
    font-size: 12px;
}

.asset-tx-time {
    width: 100%;
    font-size: 11px;
    color: #999;
    padding-left: 22px;
}
"#;
