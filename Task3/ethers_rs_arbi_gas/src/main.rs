use anyhow::{Context, Result};
use ethers::prelude::*;
use ethers::utils::{format_ether, format_units};


const SIMPLE_TX_GAS: u64 = 21_000;
#[derive(Debug)]
struct Quote {
    gp_wei: U256,gl: U256,fee_wei: U256,gp_gwei: String,fee_eth: String, 
}
/// provider.get_gas_price() 动态获取 gas_pricegas_limit = 21000（基础转账）fee_wei = gas_price * gas_limit
async fn quote_fee(endpoint: &str) -> Result<Quote> {
    let rpc = Provider::<Http>::try_from(endpoint)
        .with_context(|| format!("RPC URL 无法解析或初始化 Provider: {endpoint}"))?;

    let gp_wei = rpc
        .get_gas_price()
        .await
        .context("RPC 调用失败：get_gas_price")?;

    let gl = U256::from(SIMPLE_TX_GAS);
    let fee_wei = gp_wei * gl;

    let gp_gwei = format_units(gp_wei, "gwei").context("format_units 失败：wei -> gwei")?;
    let fee_eth = format_ether(fee_wei);

    Ok(Quote {
        gp_wei,gl,fee_wei,gp_gwei,fee_eth,
    })
}


fn print_one_liner(endpoint: &str, q: &Quote) {
    println!(
        "RPC={endpoint} | gas_price={} gwei ({} wei) | gas_limit={} | fee={} wei (~{} ETH)",
        q.gp_gwei, q.gp_wei, q.gl, q.fee_wei, q.fee_eth
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    let endpoint = "https://arbitrum-sepolia-rpc.publicnode.com";
    let q = quote_fee(endpoint).await?;
    print_one_liner(endpoint, &q);
    Ok(())
}
