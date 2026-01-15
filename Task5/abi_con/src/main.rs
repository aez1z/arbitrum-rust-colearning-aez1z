use ethers::{
    abi::Abi,
    contract::Contract,
    providers::{Http, Provider, Middleware},
    types::Address,
};
use std::{str::FromStr, sync::Arc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rpc_url = "https://sepolia-rollup.arbitrum.io/rpc";
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let client = Arc::new(provider);


    let contract_address = "0xc2b43a3efaaff6917955d27ca014fa03c7a537af";
    let target = Address::from_str(contract_address)?;

    let code = client.get_code(target, None).await?;
    if code.is_empty() {
        return Err("❌ 该地址在当前链上不是合约（get_code=0x）".into());
    }

    // 2) 最小 ABI：只写真实存在的只读函数
    let abi_json = r#"
    [
      {
        "inputs": [],
        "name": "greet",
        "outputs": [{"internalType":"string","name":"","type":"string"}],
        "stateMutability":"view",
        "type":"function"
      },
      {
        "inputs": [],
        "name": "l1Target",
        "outputs": [{"internalType":"address","name":"","type":"address"}],
        "stateMutability":"view",
        "type":"function"
      }
    ]
    "#;

    let abi: Abi = serde_json::from_str(abi_json)?;
    let c = Contract::new(target, abi, client);

    println!("开始合约查询交互");
    println!("----------------------------------------");

    // 调 greet()
    let greet: String = c.method("greet", ())?.call().await?;
    println!("greet() => {}", greet);

    // 调 l1Target()
    let l1_target: Address = c.method("l1Target", ())?.call().await?;
    println!("l1Target() => {:?}", l1_target);

    Ok(())
}
