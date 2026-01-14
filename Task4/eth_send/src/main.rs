use dotenvy::dotenv;
use ethers::prelude::{
    Address, Http, LocalWallet, Provider, SignerMiddleware, NameOrAddress, U256,
};
use ethers::providers::Middleware;
use ethers::signers::Signer;
use ethers::types::{BlockNumber, H256, TransactionReceipt};
use ethers::types::transaction::eip1559::Eip1559TransactionRequest;
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::utils::{format_ether, parse_ether};
use eyre::{Context, Result};
use std::{
    cmp::max,
    env,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

const ARBITRUM_SEPOLIA_RPC: &str = "https://sepolia-rollup.arbitrum.io/rpc";
const ARB_SEPOLIA_CHAIN_ID: u64 = 421_614;
const TX_WAIT_TIMEOUT: u64 = 60;

fn validate_eth_address(address: &str) -> Result<Address> {
    Address::from_str(address).wrap_err_with(|| format!("❌ 地址格式不合法: {address}"))
}

fn load_private_key_with_chain_id(chain_id: u64) -> Result<LocalWallet> {
    dotenv().ok();
    let pk_str = env::var("ARBITRUM_TEST_PRIVATE_KEY")
        .context("❌ 请设置环境变量 ARBITRUM_TEST_PRIVATE_KEY")?;
    let pk_str = pk_str.trim();

    let w: LocalWallet = pk_str
        .parse::<LocalWallet>()
        .context("❌ 私钥格式错误（需 64 位十六进制字符串，可带 0x）")?;
    Ok(w.with_chain_id(chain_id))
}

async fn wait_for_tx_confirmation(
    provider: &Provider<Http>,
    tx_hash: H256,
    timeout_secs: u64,
) -> Result<Option<TransactionReceipt>> {
    let start = Instant::now();
    loop {
        if start.elapsed().as_secs() > timeout_secs {
            return Ok(None);
        }
        if let Some(receipt) = provider.get_transaction_receipt(tx_hash).await? {
            return Ok(Some(receipt));
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Arbitrum Sepolia ETH 转账脚本（ethers-rs 2.0.14）");
    println!("===========================================\n");

    // 1) Provider
    println!("🔍 连接 RPC...");
    let provider = Provider::<Http>::try_from(ARBITRUM_SEPOLIA_RPC)
        .wrap_err("❌ RPC 初始化失败")?
        .interval(Duration::from_millis(250));
    println!("✅ RPC: {}", ARBITRUM_SEPOLIA_RPC);

    // 2) 确认 chain_id
    let rpc_chain_id = provider.get_chainid().await?.as_u64();
    println!("🔗 RPC chain_id: {}", rpc_chain_id);
    if rpc_chain_id != ARB_SEPOLIA_CHAIN_ID {
        return Err(eyre::eyre!(
            "❌ RPC 链ID不匹配：当前 {}，预期 {}",
            rpc_chain_id,
            ARB_SEPOLIA_CHAIN_ID
        ));
    }

    // 3) Wallet + client
    println!("🔐 加载私钥...");
    let wallet = load_private_key_with_chain_id(rpc_chain_id)?;
    let from_addr = wallet.address();
    println!("✅ 发起地址: {:#x}", from_addr);

    let client = Arc::new(SignerMiddleware::new(provider.clone(), wallet));

    // 4) To 地址
    let to_addr_str = env::var("ARBITRUM_RECEIVE_ADDRESS")
        .context("❌ 请设置环境变量 ARBITRUM_RECEIVE_ADDRESS")?;
    let to_addr = validate_eth_address(&to_addr_str)?;
    println!("✅ 接收地址: {:#x}", to_addr);

    // 5) 金额
    let amount_eth_str = env::var("TRANSFER_AMOUNT_ETH")
        .context("❌ 请设置环境变量 TRANSFER_AMOUNT_ETH（如 0.001）")?;
    let amount_eth: f64 = amount_eth_str
        .parse()
        .context("❌ TRANSFER_AMOUNT_ETH 需为数字（如 0.001）")?;
    let amount_wei = parse_ether(amount_eth).context("❌ 金额解析失败")?;
    println!("✅ 转账金额: {} ETH ({} wei)", amount_eth, amount_wei);

    // 6) baseFee + 1559 fee（避免 maxFee < baseFee）
    println!("\n💸 获取 EIP-1559 费用建议...");
    let latest_block = provider
        .get_block(BlockNumber::Latest)
        .await?
        .ok_or_else(|| eyre::eyre!("❌ 拿不到最新区块"))?;
    let base_fee = latest_block
        .base_fee_per_gas
        .ok_or_else(|| eyre::eyre!("❌ 最新区块没有 base_fee_per_gas"))?;

    let (suggest_max_fee, suggest_tip) = provider
        .estimate_eip1559_fees(None)
        .await
        .context("❌ estimate_eip1559_fees 失败")?;

    let min_need = base_fee + suggest_tip;
    let final_max_fee = max(suggest_max_fee, min_need) * 12 / 10; // +20% buffer

    println!("baseFee           : {} wei", base_fee);
    println!("suggest tip       : {} wei", suggest_tip);
    println!("suggest maxFee    : {} wei", suggest_max_fee);
    println!("final maxFee(+20%): {} wei", final_max_fee);

    // 7) 先构造 tx（先不填 gas，让 estimate_gas 来算）
    let mut tx1559 = Eip1559TransactionRequest {
        from: Some(from_addr),
        to: Some(NameOrAddress::Address(to_addr)),
        value: Some(amount_wei),
        // gas 先 None，避免你固定 21000 导致 intrinsic gas too low
        gas: None,
        max_fee_per_gas: Some(final_max_fee),
        max_priority_fee_per_gas: Some(suggest_tip),
        data: None, // ✅ 明确纯转账，不带 data
        ..Default::default()
    };

    // 8) 估算 gas（关键修复点）
    println!("\n⛽ 估算 gas...");
    let typed_for_estimate: TypedTransaction = tx1559.clone().into();
    let gas_est = client
        .estimate_gas(&typed_for_estimate, None)
        .await
        .context("❌ estimate_gas 失败")?;
    let gas_limit = gas_est * 12 / 10; // +20% buffer
    tx1559.gas = Some(gas_limit);

    println!("估算 gas: {}", gas_est);
    println!("最终 gas(+20%): {}", gas_limit);

    // 9) 余额检查（按 gas 上限估算）
    println!("\n💰 检查余额...");
    let balance_wei = provider.get_balance(from_addr, None).await?;
    println!("✅ 当前余额: {} wei ≈ {} ETH", balance_wei, format_ether(balance_wei));

    let est_gas_cost = gas_limit * final_max_fee;
    let total_required = amount_wei + est_gas_cost;

    println!("✅ 预估 gas 成本(上限): {} wei ≈ {} ETH", est_gas_cost, format_ether(est_gas_cost));
    println!("✅ 预估总花费(上限): {} wei ≈ {} ETH", total_required, format_ether(total_required));

    if balance_wei < total_required {
        return Err(eyre::eyre!(
            "❌ 余额不足！需要 ≈{} ETH（含 gas 上限），当前仅 ≈{} ETH",
            format_ether(total_required),
            format_ether(balance_wei)
        ));
    }

    // 10) 发送交易
    println!("\n📤 发送交易...");
    let pending = client
        .send_transaction(tx1559, None)
        .await
        .context("❌ 交易发送失败")?;

    let tx_hash: H256 = *pending;
    println!("\n🎉 交易已广播！");
    println!("📜 交易哈希: {:#x}", tx_hash);
    println!("🔍 浏览器查看: https://sepolia.arbiscan.io/tx/{:#x}\n", tx_hash);

    // 11) 等待确认
    println!("⌛ 等待交易上链确认（最多{}秒）...", TX_WAIT_TIMEOUT);
    match wait_for_tx_confirmation(&provider, tx_hash, TX_WAIT_TIMEOUT).await? {
        Some(receipt) => {
            println!("✅ 交易已上链！");
            println!("📦 区块高度: {:?}", receipt.block_number);
            println!("🔥 实际消耗Gas: {:?}", receipt.gas_used);
            println!("✅ 状态: {:?}", receipt.status);
        }
        None => {
            println!("⚠️  未在 {} 秒内确认，请在浏览器手动查看状态", TX_WAIT_TIMEOUT);
        }
    }

    println!("\n✅ 脚本执行完成！");
    Ok(())
}
