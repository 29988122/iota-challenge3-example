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
use move_core_types::{
    language_storage::{TypeTag, StructTag},
    account_address::AccountAddress,
    identifier::Identifier as MoveIdentifier,
};
use std::str::FromStr;

const PACKAGE_ID: &str = "0xc6f00a2b5ec2d161442b305dcb307ba914e20c5268ec931bd14d7ea3454b262b";
const TREASURY_CAP_ID: &str = "0x11d7aacb27eb65063dbb6ce0fa07f7807316c5e77763c6f2356d1bd3a34a2741";
const SHARED_COUNTER_ID: &str = "0xc3716689fa16bd8d8bf33ce1036b00740c8818ab9826dba846ef736501fd34b7";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Challenge 3: Starting full transaction flow...");

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
    println!("✅ Found {} coins", coins.data.len());
    
    println!("⛽ Getting gas price...");
    let gas_price = client.read_api().get_reference_gas_price().await?;
    println!("✅ Gas price: {}", gas_price);
    
    println!("🏗️ Building transaction for mint -> merge -> split -> get_flag...");
    let mut ptb = ProgrammableTransactionBuilder::new();
    
    // Add treasury cap as input
    let treasury_cap_arg = ptb.input(CallArg::Object(ObjectArg::SharedObject {
        id: ObjectID::from_str(TREASURY_CAP_ID)?,
        initial_shared_version: iota_sdk::types::base_types::SequenceNumber::from_u64(6286155),
        mutable: true,
    }))?;
    
    // Add counter as input
    let counter_arg = ptb.input(CallArg::Object(ObjectArg::SharedObject {
        id: ObjectID::from_str(SHARED_COUNTER_ID)?,
        initial_shared_version: iota_sdk::types::base_types::SequenceNumber::from_u64(6286155),
        mutable: true,
    }))?;

    // Define the MINTCOIN type tag
    let mintcoin_type_tag = TypeTag::Struct(Box::new(StructTag {
        address: AccountAddress::from_str(PACKAGE_ID)?,
        module: MoveIdentifier::new("mintcoin")?,
        name: MoveIdentifier::new("MINTCOIN")?,
        type_params: vec![],
    }));

    // Step 1: Mint first coin
    let mint_result1 = ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str(PACKAGE_ID)?,
        module: Identifier::new("mintcoin")?,
        function: Identifier::new("mint_coin")?,
        type_arguments: vec![],
        arguments: vec![treasury_cap_arg],
    })));

    // Step 2: Mint second coin
    let mint_result2 = ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str(PACKAGE_ID)?,
        module: Identifier::new("mintcoin")?,
        function: Identifier::new("mint_coin")?,
        type_arguments: vec![],
        arguments: vec![treasury_cap_arg],
    })));

    // Step 3: Mint third coin
    let mint_result3 = ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str(PACKAGE_ID)?,
        module: Identifier::new("mintcoin")?,
        function: Identifier::new("mint_coin")?,
        type_arguments: vec![],
        arguments: vec![treasury_cap_arg],
    })));

    // Step 4: Merge coins - join coin2 into coin1
    ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str("0x2")?,
        module: Identifier::new("coin")?,
        function: Identifier::new("join")?,
        type_arguments: vec![mintcoin_type_tag.clone()],
        arguments: vec![mint_result1, mint_result2],
    })));

    // Step 5: Merge coins - join coin3 into coin1 (now contains coin1+coin2+coin3)
    ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str("0x2")?,
        module: Identifier::new("coin")?,
        function: Identifier::new("join")?,
        type_arguments: vec![mintcoin_type_tag.clone()],
        arguments: vec![mint_result1, mint_result3],
    })));

    // Step 6: Split coin to get exactly 5 units
    let amount_5 = ptb.input(CallArg::Pure(bcs::to_bytes(&5u64)?))?;
    let split_result = ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str("0x2")?,
        module: Identifier::new("coin")?,
        function: Identifier::new("split")?,
        type_arguments: vec![mintcoin_type_tag.clone()],
        arguments: vec![mint_result1, amount_5],
    })));

    // Step 7: Call get_flag with the counter and the coin of value 5
    ptb.command(Command::MoveCall(Box::new(ProgrammableMoveCall {
        package: ObjectID::from_str(PACKAGE_ID)?,
        module: Identifier::new("mintcoin")?,
        function: Identifier::new("get_flag")?,
        type_arguments: vec![],
        arguments: vec![counter_arg, split_result],
    })));
    
    println!("✅ Transaction built");
    
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
    println!("🔗 Transaction Digest: {:?}", response.digest);
    
    if let Some(effects) = &response.effects {
        println!("✨ Transaction Effects: {:#?}", effects);
        println!("🎉 Transaction completed! Check the effects above for results.");
    }

    Ok(())
}