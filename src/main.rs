use iota_sdk::{
    IotaClientBuilder,
    types::{
        base_types::ObjectID,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{Command, TransactionData, CallArg, ObjectArg, ProgrammableMoveCall},
        Identifier,
    },
    rpc_types::{IotaTransactionBlockResponseOptions},
};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use shared_crypto::intent::Intent;
use std::str::FromStr;

const PACKAGE_ID: &str = "0xc6f00a2b5ec2d161442b305dcb307ba914e20c5268ec931bd14d7ea3454b262b";
const TREASURY_CAP_ID: &str = "0x11d7aacb27eb65063dbb6ce0fa07f7807316c5e77763c6f2356d1bd3a34a2741";
const SHARED_COUNTER_ID: &str = "0xc3716689fa16bd8d8bf33ce1036b00740c8818ab9826dba846ef736501fd34b7";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Challenge 3: Complete mint → merge → split → get_flag flow");
    println!("📝 Implementing the full solution in Rust SDK");
    
    println!("🔗 Connecting to IOTA testnet...");
    let client = IotaClientBuilder::default()
        .build("https://api.testnet.iota.cafe")
        .await?;
    println!("✅ Connected to IOTA testnet");

    println!("🔑 Loading keystore...");
    let keystore_path = dirs::home_dir()
        .unwrap()
        .join(".iota")
        .join("iota_config")
        .join("iota.keystore");
    
    let keystore = FileBasedKeystore::new(&keystore_path)?;
    let addresses = keystore.addresses();
    if addresses.is_empty() {
        return Err("No addresses in keystore".into());
    }
    let sender_address = addresses[0];
    println!("✅ Using address: {}", sender_address);

    println!("💰 Getting coins...");
    let coins = client
        .coin_read_api()
        .get_coins(sender_address, None, None, None)
        .await?;
    
    if coins.data.is_empty() {
        return Err("No coins found".into());
    }
    let gas_coin = &coins.data[0];
    println!("✅ Found {} coins", coins.data.len());
    
    println!("⛽ Getting gas price...");
    let gas_price = client.read_api().get_reference_gas_price().await?;
    println!("✅ Gas price: {}", gas_price);
    
    println!("🏗️ Building complete transaction...");
    let mut ptb = ProgrammableTransactionBuilder::new();
    
    // 準備共享對象
    let treasury_cap_arg = ptb.input(CallArg::Object(ObjectArg::SharedObject {
        id: ObjectID::from_str(TREASURY_CAP_ID)?,
        initial_shared_version: iota_sdk::types::base_types::SequenceNumber::from_u64(6286155),
        mutable: true,
    }))?;
    
    let counter_arg = ptb.input(CallArg::Object(ObjectArg::SharedObject {
        id: ObjectID::from_str(SHARED_COUNTER_ID)?,
        initial_shared_version: iota_sdk::types::base_types::SequenceNumber::from_u64(6286155),
        mutable: true,
    }))?;
    
    println!("🪙 Step 1: Minting 3 coins (each worth 2)...");
    // Step 1: Mint 3 coins (每個價值 2)
    let coin1 = ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str(PACKAGE_ID)?,
        module: Identifier::new("mintcoin")?,
        function: Identifier::new("mint_coin")?,
        type_arguments: vec![],
        arguments: vec![treasury_cap_arg],
    })));
    
    let coin2 = ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str(PACKAGE_ID)?,
        module: Identifier::new("mintcoin")?,
        function: Identifier::new("mint_coin")?,
        type_arguments: vec![],
        arguments: vec![treasury_cap_arg],
    })));
    
    let coin3 = ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str(PACKAGE_ID)?,
        module: Identifier::new("mintcoin")?,
        function: Identifier::new("mint_coin")?,
        type_arguments: vec![],
        arguments: vec![treasury_cap_arg],
    })));
    
    println!("🔗 Step 2: Merging coins using pay::join...");
    // Step 2: 使用 pay::join 來合併 coins
    // 創建一個包含 coin2 和 coin3 的向量
    let make_vec = ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str("0x1")?,
        module: Identifier::new("vector")?,
        function: Identifier::new("empty")?,
        type_arguments: vec![],
        arguments: vec![],
    })));
    
    ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str("0x1")?,
        module: Identifier::new("vector")?,
        function: Identifier::new("push_back")?,
        type_arguments: vec![],
        arguments: vec![make_vec, coin2],
    })));
    
    ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str("0x1")?,
        module: Identifier::new("vector")?,
        function: Identifier::new("push_back")?,
        type_arguments: vec![],
        arguments: vec![make_vec, coin3],
    })));
    
    ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str("0x2")?,
        module: Identifier::new("pay")?,
        function: Identifier::new("join_vec")?,
        type_arguments: vec![],
        arguments: vec![coin1, make_vec],
    })));
    
    println!("✂️ Step 3: Splitting coin to get value 5...");
    // Step 3: Split coin to get exactly 5 value
    let split_amount = ptb.input(CallArg::Pure(bcs::to_bytes(&5u64)?))?;
    let split_coin = ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str("0x2")?,
        module: Identifier::new("pay")?,
        function: Identifier::new("split")?,
        type_arguments: vec![],
        arguments: vec![coin1, split_amount],
    })));
    
    println!("🏆 Step 4: Calling get_flag with coin value 5...");
    // Step 4: Call get_flag with counter and split_coin (價值為 5)
    ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str(PACKAGE_ID)?,
        module: Identifier::new("mintcoin")?,
        function: Identifier::new("get_flag")?,
        type_arguments: vec![],
        arguments: vec![counter_arg, split_coin],
    })));
    
    println!("✅ Transaction built with complete flow");
    
    // 構建並執行交易
    let tx_data = TransactionData::new_programmable(
        sender_address,
        vec![gas_coin.object_ref()],
        ptb.finish(),
        50_000_000,
        gas_price,
    );
    
    println!("✍️ Signing transaction...");
    let signature = keystore.sign_secure(&sender_address, &tx_data, Intent::iota_transaction())?;
    println!("✅ Transaction signed");
    
    println!("📤 Executing transaction...");
    let response = client
        .quorum_driver_api()
        .execute_transaction_block(
            iota_sdk::types::transaction::Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(iota_sdk::types::quorum_driver_types::ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;
    
    println!("✅ Transaction executed!");
    println!("Transaction: {:?}", response.digest);
    
    if let Some(effects) = &response.effects {
        println!("📊 Transaction Effects:");
        println!("{:#?}", effects);
        println!("🏆 Challenge 3 transaction completed with Rust SDK!");
        println!("💡 Check the effects above for created Flag objects");
    }

    Ok(())
}