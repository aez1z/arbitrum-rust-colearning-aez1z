use anyhow::{anyhow, Context, Result};
use ethers::prelude::*;
use ethers::utils::format_ether;
use std::{env, str::FromStr};

const DEFAULT_RPC: &str = "https://sepolia-rollup.arbitrum.io/rpc";

#[derive(Debug)]
struct Config {
    address: Address,
    rpc_url: String,
}

impl Config {
    fn from_args() -> Result<Self> {
        // 用法：
        // cargo run -- <ADDRESS> [RPC_URL]
        let mut args = env::args().skip(1);

        let address_str = args
            .next()
            .ok_or_else(|| anyhow!("缺少参数：地址。\n用法: balance <ADDRESS> [RPC_URL]"))?;

        let rpc_url = args.next().unwrap_or_else(|| DEFAULT_RPC.to_string());

        let address = Address::from_str(&address_str)
            .with_context(|| format!("地址格式不正确: {address_str}"))?;

        Ok(Self { address, rpc_url })
    }
}

async fn fetch_balance_wei(provider: &Provider<Http>, address: Address) -> Result<U256> {
    provider
        .get_balance(address, None)
        .await
        .context("RPC 调用失败：get_balance")
}

async fn query_balance(config: &Config) -> Result<(U256, String)> {
    let provider = Provider::<Http>::try_from(config.rpc_url.as_str())
        .with_context(|| format!("RPC URL 无法解析或初始化 Provider: {}", config.rpc_url))?;

    let wei = fetch_balance_wei(&provider, config.address).await?;
    let eth = format_ether(wei);

    Ok((wei, eth))
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    let (wei, eth) = query_balance(&config).await?;

    // 输出加小图标
    println!("👛 Address : {:?}", config.address);
    println!("🌐 RPC     : {}", config.rpc_url);
    println!("💰 Balance : {} ETH", eth);
    println!("🧾 Wei     : {}", wei);
    println!("✅ {wei} wei = {eth} ETH");

    Ok(())
}
