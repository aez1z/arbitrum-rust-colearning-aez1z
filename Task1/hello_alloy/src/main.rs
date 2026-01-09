use alloy::providers::{Provider, ProviderBuilder};
use alloy::primitives::Address;
use alloy::sol;
use std::error::Error;

sol! {
    #[sol(rpc)]
    contract HelloWeb3 {
        function hello_web3() pure public returns (string memory);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 1) 初始化 provider
    let rpc_url = "https://arbitrum-sepolia-rpc.publicnode.com".parse()?;
    let provider = ProviderBuilder::new().connect_http(rpc_url);

    // 2) 查询最新区块
    let latest_block = provider.get_block_number().await?;
    println!("Latest block number: {}", latest_block);

    // 3) 调用合约方法（只读 call）
    let contract_address: Address =
        "0x3f1f78ED98Cd180794f1346F5bD379D5Ec47DE90".parse()?;

    // 注意：这里把 provider 以引用方式传给合约实例，避免 Clone/Move 问题
    let contract = HelloWeb3::new(contract_address, &provider);

    let result = contract.hello_web3().call().await?;
    println!("合约返回: {}", result);

    Ok(())
}
