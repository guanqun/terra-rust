use anyhow::Result;
use clap::{Arg, ArgMatches, Subcommand};
use dotenv::dotenv;
use secp256k1::Secp256k1;
use terra_rust_api::core_types::Coin;
use terra_rust_api::messages::wasm::{MsgInstantiateContract, MsgMigrateContract};
use terra_rust_api::{Message, MsgExecuteContract, Terra};
use terra_rust_cli::cli_helpers;
/// VERSION number of package
pub const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");
/// NAME of package
pub const NAME: Option<&'static str> = option_env!("CARGO_PKG_NAME");

#[derive(Subcommand)]
#[allow(clippy::upper_case_acronyms)]
enum TerraCommands {
    Migrate {
        contract: String,
        wasm: String,
        migrate: Option<String>,
    },
    Exec {
        contract: String,
        exec: String,
        coins: Option<String>,
    },
    Store {
        wasm: String,
    },
    Instantiate {
        wasm: String,
        json: String,
        admin: Option<String>,
        coins: Option<String>,
    },
    Query {
        contract: String,
        query: String,
    },
}
async fn run(args: Vec<String>) -> Result<()> {
    let memo = Some(format!(
        "PFC-{}/{}",
        NAME.unwrap_or("TERRARUST"),
        VERSION.unwrap_or("DEV")
    ));
    let cli: clap::Command = cli_helpers::gen_cli("terra", "cargo-terra").args(&[
        Arg::new("retries")
            .long("retries")
            .takes_value(true)
            .value_name("retries")
            .required(false)
            .default_value("5")
            .help("amount of times to retry fetching hash"),
        Arg::new("sleep")
            .long("sleep")
            .takes_value(true)
            .value_name("sleep")
            .required(false)
            .default_value("3")
            .help("amount of seconds before retying to fetch hash"),
    ]);
    let matches: ArgMatches = TerraCommands::augment_subcommands(cli).get_matches_from(args);

    let sleep = cli_helpers::get_arg_value(&matches, "sleep")?.parse::<u64>()?;
    let retries = cli_helpers::get_arg_value(&matches, "retries")?.parse::<usize>()?;

    match matches.subcommand() {
        Some(("migrate", migrate)) => {
            let contract = cli_helpers::get_arg_value(migrate, "contract")?;

            if !contract.starts_with("terra1") {
                anyhow::bail!("invalid contract address");
            }
            let terra = cli_helpers::lcd_from_args(&matches).await?;
            let secp = Secp256k1::new();
            let private = cli_helpers::get_private_key(&secp, &matches)?;
            let wasm = cli_helpers::get_arg_value(migrate, "wasm")?;
            let code_id = if let Ok(code_id) = wasm.parse::<u64>() {
                code_id
            } else {
                let hash = terra
                    .wasm()
                    .store(&secp, &private, wasm, memo.clone())
                    .await?
                    .txhash;
                let code_id = get_attribute_tx(
                    &terra,
                    &hash,
                    retries,
                    tokio::time::Duration::from_secs(sleep),
                    "store_code",
                    "code_id",
                )
                .await?;
                code_id.parse::<u64>()?
            };

            let json = if let Some(migrate_json) = migrate.value_of("migrate") {
                let json_block = cli_helpers::get_json_block(migrate_json)?.to_string();
                Some(MsgMigrateContract::replace_parameters(
                    &private.public_key(&secp).account()?,
                    contract,
                    code_id,
                    &json_block,
                ))
            } else {
                None
            };
            let hash = terra
                .wasm()
                .migrate(&secp, &private, contract, code_id, json, memo)
                .await?
                .txhash;
            let tx = terra
                .tx()
                .get_and_wait_v1(&hash, retries, tokio::time::Duration::from_secs(sleep))
                .await?;
            let codes = tx
                .tx_response
                .get_attribute_from_logs("migrate_contract", "contract_address");
            let contract = if let Some(code) = codes.first() {
                code.1.clone()
            } else {
                panic!(
                    "{}/{} not present in TX log",
                    "migrate_contract", "contract_address"
                );
            };

            let codes = tx
                .tx_response
                .get_attribute_from_logs("migrate_contract", "code_id");
            let code_id = if let Some(code) = codes.first() {
                code.1.clone()
            } else {
                panic!("{}/{} not present in TX log", "migrate_contract", "code_id");
            };

            println!("Contract: {} Migrated to {}", contract, code_id);
        }
        Some(("instantiate", instantiate)) => {
            let terra = cli_helpers::lcd_from_args(&matches).await?;
            let secp = Secp256k1::new();
            let private = cli_helpers::get_private_key(&secp, &matches)?;
            let wasm = cli_helpers::get_arg_value(instantiate, "wasm")?;
            let coins = if let Some(coin_str) = instantiate.value_of("coins") {
                Coin::parse_coins(coin_str)?
            } else {
                vec![]
            };
            let code_id = if let Ok(code_id) = wasm.parse::<u64>() {
                code_id
            } else {
                let hash = terra
                    .wasm()
                    .store(&secp, &private, wasm, memo.clone())
                    .await?
                    .txhash;
                let code_id = get_attribute_tx(
                    &terra,
                    &hash,
                    retries,
                    tokio::time::Duration::from_secs(sleep),
                    "store_code",
                    "code_id",
                )
                .await?;
                code_id.parse::<u64>()?
            };
            let admin: Option<String> = if let Some(admin) = instantiate.value_of("admin") {
                if admin.starts_with("terra1") {
                    Some(admin.to_string())
                } else if admin == "same" {
                    Some(private.public_key(&secp).account()?)
                } else if admin == "none" {
                    todo!("Admin of none/empty not supported")
                } else {
                    let wallet = cli_helpers::wallet_from_args(&matches)?;
                    let seed = matches.value_of("seed");
                    let admin_key = wallet.get_public_key(&secp, admin, seed)?;
                    let admin_account = admin_key.account()?;
                    Some(admin_account)
                }
            } else {
                todo!("Admin of none/empty not supported")
            };

            let init_json = cli_helpers::get_arg_value(instantiate, "json")?;
            let json = cli_helpers::get_json_block(init_json)?.to_string();
            let init_json_parsed = MsgInstantiateContract::replace_parameters(
                &private.public_key(&secp).account()?,
                admin.clone(),
                code_id,
                &json,
            );

            let hash = terra
                .wasm()
                .instantiate(
                    &secp,
                    &private,
                    code_id,
                    init_json_parsed,
                    coins,
                    admin,
                    memo,
                )
                .await?
                .txhash;
            let tx = terra
                .tx()
                .get_and_wait_v1(&hash, retries, tokio::time::Duration::from_secs(sleep))
                .await?;
            let codes = tx
                .tx_response
                .get_attribute_from_logs("instantiate_contract", "contract_address");
            let contract = if let Some(code) = codes.first() {
                code.1.clone()
            } else {
                panic!(
                    "{}/{} not present in TX log",
                    "migrate_contract", "contract_address"
                );
            };

            let codes = tx
                .tx_response
                .get_attribute_from_logs("instantiate_contract", "code_id");
            let code_id = if let Some(code) = codes.first() {
                code.1.clone()
            } else {
                panic!("{}/{} not present in TX log", "migrate_contract", "code_id");
            };

            println!("Contract: {} running  code {}", contract, code_id);
        }
        Some(("store", store)) => {
            let terra = cli_helpers::lcd_from_args(&matches).await?;
            let secp = Secp256k1::new();
            let private = cli_helpers::get_private_key(&secp, &matches)?;
            let wasm = cli_helpers::get_arg_value(store, "wasm")?;

            let hash = terra
                .wasm()
                .store(&secp, &private, wasm, memo.clone())
                .await?
                .txhash;
            let code_id = get_attribute_tx(
                &terra,
                &hash,
                retries,
                tokio::time::Duration::from_secs(sleep),
                "store_code",
                "code_id",
            )
            .await?;

            println!("Contract: stored with code {}", code_id);
        }
        Some(("exec", exec)) => {
            let contract = cli_helpers::get_arg_value(exec, "contract")?;

            if !contract.starts_with("terra1") {
                anyhow::bail!("invalid contract address");
            }
            let terra = cli_helpers::lcd_from_args(&matches).await?;
            let secp = Secp256k1::new();
            let private = cli_helpers::get_private_key(&secp, &matches)?;
            let coins = if let Some(coin_str) = exec.value_of("coins") {
                Coin::parse_coins(coin_str)?
            } else {
                vec![]
            };
            let exec_str = cli_helpers::get_arg_value(exec, "exec")?;
            let json = cli_helpers::get_json_block(exec_str)?;
            let exec_message = MsgExecuteContract::create_from_value(
                &private.public_key(&secp).account()?,
                contract,
                &json,
                &coins,
            )?;
            let messages: Vec<Message> = vec![exec_message];

            let resp = terra
                .submit_transaction_sync(&secp, &private, messages, memo)
                .await?
                .txhash;
            println!("{}", resp);
        }
        Some(("query", query)) => {
            let contract = cli_helpers::get_arg_value(query, "contract")?;

            if !contract.starts_with("terra1") {
                anyhow::bail!("invalid contract address");
            }
            let terra = cli_helpers::lcd_no_tx_from_args(&matches)?;
            let query_str = cli_helpers::get_arg_value(query, "query")?;
            let query_json = cli_helpers::get_json_block(query_str)?.to_string();
            let result = terra
                .wasm()
                .query::<serde_json::Value>(contract, &query_json, None)
                .await?;

            println!("{}", serde_json::to_string_pretty(&result)?)
        }
        _ => {
            println!("try --help")
        }
    }
    Ok(())
}

async fn get_attribute_tx(
    terra: &Terra,
    hash: &str,
    retries: usize,
    sleep: tokio::time::Duration,
    event_type: &str,
    attribute_key: &str,
) -> Result<String> {
    let tx = terra.tx().get_and_wait_v1(hash, retries, sleep).await?;
    let codes = tx
        .tx_response
        .get_attribute_from_logs(event_type, attribute_key);
    if let Some(code) = codes.first() {
        Ok(code.1.clone())
    } else {
        panic!("{}/{} not present in TX log", event_type, attribute_key)
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::init();
    // in case we are invoked by cargo-terra
    let mut args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "terra" {
        args.remove(1);
    }

    if let Err(ref err) = run(args).await {
        log::error!("{}", err);
        err.chain()
            .skip(1)
            .for_each(|cause| log::error!("because: {}", cause));

        // The backtrace is not always generated. Try to run this example
        // with `$env:RUST_BACKTRACE=1`.
        //    if let Some(backtrace) = e.backtrace() {
        //        log::debug!("backtrace: {:?}", backtrace);
        //    }

        ::std::process::exit(1);
    }
}
