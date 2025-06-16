use iota_sdk::{
    IotaClientBuilder,
    types::{
        base_types::{ObjectID, IotaAddress},
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{Command, TransactionData, CallArg, ObjectArg, ProgrammableMoveCall},
        Identifier,
    },
    rpc_types::IotaTransactionBlockResponseOptions,
};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use shared_crypto::intent::Intent;
use move_core_types::{
    language_storage::{TypeTag, StructTag},
    account_address::AccountAddress,
    identifier::Identifier as MoveIdentifier,
};
use std::str::FromStr;
use std::time::Duration;
use bcs;

const PACKAGE_ID: &str = "0xc6f00a2b5ec2d161442b305dcb307ba914e20c5268ec931bd14d7ea3454b262b";
const TREASURY_CAP_ID: &str = "0x11d7aacb27eb65063dbb6ce0fa07f7807316c5e77763c6f2356d1bd3a34a2741";
const SHARED_COUNTER_ID: &str = "0xc3716689fa16bd8d8bf33ce1036b00740c8818ab9826dba846ef736501fd34b7";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Challenge 3: Starting multi-transaction flow...");

    println!("🔗 Connecting to IOTA testnet...");
    let client = IotaClientBuilder::default()
        .build("https://api.testnet.iota.cafe")
        .await?;
    println!("✅ Connected to IOTA testnet");

    println!("🔑 Loading keystore...");
    let keystore_path = dirs::home_dir()
        .ok_or("Failed to get home directory")?
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

    println!("💰 Getting coins for gas...");
    let coins = client
        .coin_read_api()
        .get_coins(sender_address, None, None, None)
        .await?;
    
    let gas_coin = coins.data.get(0).ok_or("No coins found for gas")?;
    println!("✅ Found {} gas coins", coins.data.len());
    
    println!("⛽ Getting gas price...");
    let gas_price = client.read_api().get_reference_gas_price().await?;
    println!("✅ Gas price: {}", gas_price);

    // --- 交易 1: 鑄造三個 MINTCOIN ---
    println!("\n--- 交易 1: 鑄造 MINTCOINs ---");
    let mut ptb1 = ProgrammableTransactionBuilder::new();

    let treasury_cap_arg = ptb1.input(CallArg::Object(ObjectArg::SharedObject {
        id: ObjectID::from_str(TREASURY_CAP_ID)?,
        initial_shared_version: iota_sdk::types::base_types::SequenceNumber::from_u64(6286155),
        mutable: true,
    }))?;

    // 呼叫三次 mint_coin
    for i in 1..=3 {
        ptb1.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
            package: ObjectID::from_str(PACKAGE_ID)?,
            module: Identifier::new("mintcoin")?,
            function: Identifier::new("mint_coin")?,
            type_arguments: vec![],
            arguments: vec![treasury_cap_arg],
        })));
        println!("  - Command: mint_coin #{}", i);
    }
    
    let tx_data1 = TransactionData::new_programmable(
        sender_address,
        vec![gas_coin.object_ref()],
        ptb1.finish(),
        50_000_000,
        gas_price,
    );
    
    println!("✍️ 簽署交易 1...");
    let signature1 = keystore.sign_secure(&sender_address, &tx_data1, Intent::iota_transaction())?;
    
    println!("📤 執行交易 1...");
    let response1 = client
        .quorum_driver_api()
        .execute_transaction_block(
            iota_sdk::types::transaction::Transaction::from_data(tx_data1, vec![signature1]),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(iota_sdk::types::quorum_driver_types::ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    println!("✅ 交易 1 已執行!");
    println!("🔗 交易摘要: {:?}", response1.digest);

    if let Some(effects) = &response1.effects {
        println!("✨ 交易 1 效果: {:#?}", effects);
    }
    println!("✅ 交易 1 已發送 (請手動檢查是否成功)!");

    // --- 等待 & 尋找新鑄造的 Coins ---
    println!("\n⏳ 等待 5 秒讓網路狀態同步...");
    tokio::time::sleep(Duration::from_secs(5)).await;

    println!("🔍 尋找新鑄造的 MINTCOINs...");
    let coin_type = format!("{}::mintcoin::MINTCOIN", PACKAGE_ID);
    let mint_coins = client
        .coin_read_api()
        .get_coins(sender_address, Some(coin_type), None, None)
        .await?;

    if mint_coins.data.len() < 3 {
        return Err(format!("需要的 MINTCOIN 不足。預期 >= 3, 找到 {}", mint_coins.data.len()).into());
    }
    println!("✅ 找到 {} 個 MINTCOINs", mint_coins.data.len());

    let coin_ref1 = mint_coins.data[0].object_ref();
    let coin_ref2 = mint_coins.data[1].object_ref();
    let coin_ref3 = mint_coins.data[2].object_ref();
    
    // --- 交易 2: 合併、分割、取旗標 ---
    println!("\n--- 交易 2: 合併, 分割 & 取得旗標 ---");
    let mut ptb2 = ProgrammableTransactionBuilder::new();

    let mintcoin_type_tag = TypeTag::Struct(Box::new(StructTag {
        address: AccountAddress::from_str(PACKAGE_ID)?,
        module: MoveIdentifier::new("mintcoin")?,
        name: MoveIdentifier::new("MINTCOIN")?,
        type_params: vec![],
    }));
    
    let counter_arg = ptb2.input(CallArg::Object(ObjectArg::SharedObject {
        id: ObjectID::from_str(SHARED_COUNTER_ID)?,
        initial_shared_version: iota_sdk::types::base_types::SequenceNumber::from_u64(6286155),
        mutable: true,
    }))?;

    let coin1_arg = ptb2.input(CallArg::Object(ObjectArg::ImmOrOwnedObject(coin_ref1)))?;
    let coin2_arg = ptb2.input(CallArg::Object(ObjectArg::ImmOrOwnedObject(coin_ref2)))?;
    let coin3_arg = ptb2.input(CallArg::Object(ObjectArg::ImmOrOwnedObject(coin_ref3)))?;

    // Step A: Merge coins
    ptb2.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str("0x2")?,
        module: Identifier::new("coin")?,
        function: Identifier::new("join")?,
        type_arguments: vec![mintcoin_type_tag.clone()],
        arguments: vec![coin1_arg, coin2_arg],
    })));
    println!("  - Command: join(coin1, coin2)");

    ptb2.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str("0x2")?,
        module: Identifier::new("coin")?,
        function: Identifier::new("join")?,
        type_arguments: vec![mintcoin_type_tag.clone()],
        arguments: vec![coin1_arg, coin3_arg],
    })));
    println!("  - Command: join(coin1, coin3)");
    
    // Step B: Split the merged coin to get one coin of value 5
    let pure_data = bcs::to_bytes(&5u64)?; // The value to split off
    let value_arg = ptb2.input(CallArg::Pure(pure_data))?;
    let coin_with_5 = ptb2.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str("0x2")?, // Use standard 'coin' package
        module: Identifier::new("coin")?,
        function: Identifier::new("split")?, // Use standard 'split' function
        type_arguments: vec![mintcoin_type_tag.clone()],
        arguments: vec![coin1_arg, value_arg], // The coin to split from, and the value to take
    })));
    println!("  - Command: split(merged_coin, 5)");

    // Step D: Call get_flag with the extracted coin and transfer the returned flag
    ptb2.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str(PACKAGE_ID)?,
        module: Identifier::new("mintcoin")?,
        function: Identifier::new("get_flag")?,
        type_arguments: vec![],
        arguments: vec![counter_arg, coin_with_5],
    })));
    println!("  - Command: get_flag(counter, coin_with_5)");
    
    // Transfer the coin with value 5 after get_flag (in case it's not consumed)
    let move_address = AccountAddress::from_str(&sender_address.to_string())?;
    let addr_arg = ptb2.input(CallArg::Pure(bcs::to_bytes(&move_address)?))?;
    
    ptb2.command(Command::TransferObjects(
        vec![coin_with_5],
        addr_arg,
    ));
    println!("  - Command: transfer_objects(coin_with_5, sender)");
    
    // Step E: Transfer the remaining coin (value 1) back to self to avoid drop error
    ptb2.command(Command::TransferObjects(
        vec![coin1_arg],
        addr_arg,
    ));
    println!("  - Command: transfer_objects(remaining_coin, sender)");
    
    // Gas coin needs to be fetched again as state might have changed
    let gas_coins2 = client
        .coin_read_api()
        .get_coins(sender_address, None, None, None)
        .await?;
    let gas_coin2 = gas_coins2.data.get(0).ok_or("No coins found for gas for transaction 2")?;
    
    let tx_data2 = TransactionData::new_programmable(
        sender_address,
        vec![gas_coin2.object_ref()],
        ptb2.finish(),
        50_000_000,
        gas_price,
    );

    println!("✍️ 簽署交易 2...");
    let signature2 = keystore.sign_secure(&sender_address, &tx_data2, Intent::iota_transaction())?;

    println!("📤 執行交易 2...");
    let response2 = client
        .quorum_driver_api()
        .execute_transaction_block(
            iota_sdk::types::transaction::Transaction::from_data(tx_data2, vec![signature2]),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(iota_sdk::types::quorum_driver_types::ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    println!("✅ 交易 2 已執行!");
    println!("🔗 交易摘要: {:?}", response2.digest);

    if let Some(effects) = &response2.effects {
        println!("✨ 最終交易效果: {:#?}", effects);
        println!("\n\n🎉🎉🎉 交易 2 已完成! 請檢查上面的效果以確認是否成功! 🎉🎉🎉");
    }

    Ok(())
}