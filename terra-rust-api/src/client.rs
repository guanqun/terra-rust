// use crate::errors::{ErrorKind, Result};
use crate::client::tx_types::{TXResultAsync, TXResultSync, TxFeeResult};
use crate::core_types::{Coin, StdFee, StdSignMsg, StdSignature};
use reqwest::header::{HeaderMap, CONTENT_TYPE, USER_AGENT};
use reqwest::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};

pub mod auth;
/// Structures used in account authentication
pub mod auth_types;
/// APIs around bank module (get balances)
pub mod bank;
/// JSON Serializer/Deserializer helpers
pub mod client_types;
/// Common Structures throughout the library
pub mod core_types;
pub mod fcd;
pub mod lcd_types;
/// APIs around market operations (swap)
pub mod market;
/// APIs to perform oracle related things
pub mod oracle;
/// Structures used for Oracle APIs
pub mod oracle_types;
/// tendermint RPC
pub mod rpc;
pub mod rpc_types;
/// staking routines
pub mod staking;
/// Structures used for Staking APIs
pub mod staking_types;
/// tendermint level APIs
pub mod tendermint;
/// Structures used for Tendermint / Misc APIs
pub mod tendermint_types;
/// operations around the transaction itself
pub mod tx;
/// Structures used for sending transactions to LCD
pub mod tx_types;
/// wasm module/contract related apis
pub mod wasm;
pub mod wasm_types;

use crate::auth_types::AuthAccount;
use crate::errors::TerraRustAPIError;
use crate::errors::TerraRustAPIError::{GasPriceError, TxResultError};
use crate::messages::Message;
use crate::PrivateKey;
use crate::{AddressBook, LCDResult};

use rust_decimal_macros::dec;
use secp256k1::Secp256k1;
use secp256k1::Signing;
use std::fs::File;

/// Version # of package sent out on requests to help with debugging
const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");
/// name of package
const NAME: Option<&'static str> = option_env!("CARGO_PKG_NAME");

const NETWORK_PROD_ADDRESS_BOOK: &str = "https://network.terra.dev/addrbook.json";
const NETWORK_TEST_ADDRESS_BOOK: &str =
    "https://raw.githubusercontent.com/terra-money/testnet/master/bombay-12/addrbook.json";

/// When Submitting transactions you need to either submit gas or a fee to the validator
/// This structure is used to determine what your preferences are by default
/// Higher fees may be given preference by the validator to include the transaction in their block
#[derive(Clone, Debug)]
pub struct GasOptions {
    /// If specified the TX will use the fee specified
    pub fees: Option<Coin>,
    /// if true, the server will call the 'estimate_transaction' to get an estimate.
    /// This estimate is then multiplied by the gas_adjustment field
    pub estimate_gas: bool,
    /// your estimate of the gas to use.
    pub gas: Option<u64>,
    /// used to calculate the fee .. gas * gas_price
    pub gas_price: Option<Coin>,
    /// used to adjust the estimate
    pub gas_adjustment: Option<f64>,
}
impl GasOptions {
    /// for hard-coding of fees
    pub fn create_with_fees(fees: &str, gas: u64) -> Result<GasOptions, TerraRustAPIError> {
        Ok(GasOptions {
            fees: Coin::parse(fees)?,
            estimate_gas: false,
            gas: Some(gas),
            gas_price: None,
            gas_adjustment: None,
        })
    }
    /// for when you want the validator to give you an estimate on the amounts

    pub fn create_with_gas_estimate(
        gas_price: &str,
        gas_adjustment: f64,
    ) -> Result<GasOptions, TerraRustAPIError> {
        Ok(GasOptions {
            fees: None,
            estimate_gas: true,
            gas: None,
            gas_price: Coin::parse(gas_price)?,
            gas_adjustment: Some(gas_adjustment),
        })
    }
    pub async fn create_with_fcd(
        client: &reqwest::Client,
        fcd_url: &str,
        gas_denom: &str,
        gas_adjustment: f64,
    ) -> Result<GasOptions, TerraRustAPIError> {
        let prices = fcd::FCD::fetch_gas_prices(client, fcd_url).await?;
        if let Some(price) = prices.get(gas_denom) {
            let gas_coin = Coin::create(gas_denom, *price);
            let gas_price = Some(gas_coin);
            Ok(GasOptions {
                fees: None,
                estimate_gas: true,
                gas: None,
                gas_price,
                gas_adjustment: Some(gas_adjustment),
            })
        } else {
            Err(GasPriceError(gas_denom.into()))
        }
    }
}

/// The main structure that all API calls are generated from
#[derive(Clone)]
pub struct Terra {
    /// reqwest Client
    client: Client,
    /// The URL of the LCD
    url: String,

    /// The Chain of the network
    pub chain_id: String,
    /// Gas Options used to help with gas/fee generation of transactions
    pub gas_options: Option<GasOptions>,
    pub debug: bool,
}
impl Terra {
    /// Create a LCD client interface
    pub fn lcd_client<S: Into<String>>(
        url: S,
        chain_id: S,
        gas_options: &GasOptions,
        debug: Option<bool>,
    ) -> Terra {
        let client = reqwest::Client::new();
        Terra {
            client,
            url: url.into(),
            chain_id: chain_id.into(),
            gas_options: Some(gas_options.clone()),
            debug: debug.unwrap_or(false),
        }
    }

    /// Create a read-only / query client interface
    pub fn lcd_client_no_tx<S: Into<String>>(url: S, chain_id: S) -> Terra {
        let client = reqwest::Client::new();
        Terra {
            client,
            url: url.into(),
            chain_id: chain_id.into(),
            gas_options: None,
            debug: false,
        }
    }

    /// Auth API functions
    pub fn auth(&self) -> auth::Auth {
        auth::Auth::create(self)
    }
    /// Bank  API functions
    pub fn bank(&self) -> bank::Bank {
        bank::Bank::create(self)
    }
    /// Staking API functions
    pub fn staking(&self) -> staking::Staking {
        staking::Staking::create(self)
    }
    /// Market API functions
    pub fn market(&self) -> market::Market {
        market::Market::create(self)
    }
    /// Oracle API functions
    pub fn oracle(&self) -> oracle::Oracle {
        oracle::Oracle::create(self)
    }
    /// Tendermint (MISC) API Functions
    pub fn tendermint(&self) -> tendermint::Tendermint {
        tendermint::Tendermint::create(self)
    }
    /// TXS API Functions
    pub fn tx(&self) -> tx::TX {
        tx::TX::create(self)
    }
    /// RPC Api Functions
    pub fn rpc<'a>(&'a self, tendermint_url: &'a str) -> rpc::RPC {
        rpc::RPC::create(self, tendermint_url)
    }
    /// FCD Api Functions
    pub fn fcd<'a>(&'a self, fcd_url: &'a str) -> fcd::FCD {
        fcd::FCD::create(self, fcd_url)
    }
    /// WASM module / smart contract API Functions
    pub fn wasm(&self) -> wasm::Wasm {
        wasm::Wasm::create(self)
    }

    pub fn construct_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();

        headers.insert(
            USER_AGENT,
            format!(
                "PFC-{}/{}",
                NAME.unwrap_or("terra-rust-api"),
                VERSION.unwrap_or("-?-")
            )
            .parse()
            .unwrap(),
        );
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
        headers
    }

    /// used to send a GET command to the LCD
    pub async fn send_cmd<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        args: Option<&str>,
        height: Option<u64>,
    ) -> Result<T, TerraRustAPIError> {
        self.send_cmd_url(&self.url, path, args, height).await
    }

    /// used to send a GET command to any URL
    pub async fn send_cmd_url<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        path: &str,
        args: Option<&str>,
        height: Option<u64>,
    ) -> Result<T, TerraRustAPIError> {
        let mut request_url = match args {
            Some(a) => format!("{}{}{}", url.to_owned(), path, a),
            None => format!("{}{}", url.to_owned(), path),
        };
        if let Some(height) = height {
            // this is a bit hacky, we probably should use .query(&[("a", "b"), ("c", "d")])
            // but then it would have larger changes.
            let concat_char = if args.is_some() { "&" } else { "?" };
            let height_query = format!("{}height={}", concat_char, height);
            request_url.push_str(height_query.as_str());
        }

        if self.debug {
            log::debug!("URL={}", &request_url);
        }
        let req = self
            .client
            .get(&request_url)
            .headers(Terra::construct_headers());

        Terra::resp::<T>(&request_url, req).await
    }

    pub async fn fetch_url<T: for<'de> Deserialize<'de>>(
        client: &reqwest::Client,
        url: &str,
        path: &str,
        args: Option<&str>,
    ) -> Result<T, TerraRustAPIError> {
        let request_url = match args {
            Some(a) => format!("{}{}{}", url.to_owned(), path, a),
            None => format!("{}{}", url.to_owned(), path),
        };

        let req = client.get(&request_url).headers(Terra::construct_headers());

        Terra::resp::<T>(&request_url, req).await
    }

    /// used to send a POST with a JSON body to the LCD
    pub async fn post_cmd<R: for<'de> Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        args: &R,
    ) -> Result<T, TerraRustAPIError> {
        let request_url = format!("{}{}", self.url.to_owned(), path);

        if self.debug {
            log::debug!("URL={}", &request_url);
        }

        let req = self
            .client
            .post(&request_url)
            .headers(Terra::construct_headers())
            .json::<R>(args);

        Terra::resp::<T>(&request_url, req).await
    }

    async fn resp<T: for<'de> Deserialize<'de>>(
        request_url: &str,
        req: RequestBuilder,
    ) -> Result<T, TerraRustAPIError> {
        let response = req.send().await?;
        let status = response.status();
        if !&status.is_success() {
            let status_text = response.text().await?;
            //  eprintln!("{}", &request_url);
            log::debug!("URL={} - {}", &request_url, &status_text);
            Err(TerraRustAPIError::TerraLCDResponse(status, status_text))
        } else {
            let struct_response: T = response.json::<T>().await?;
            Ok(struct_response)
        }
    }

    /// Generate Fee structure, either by estimation method or hardcoded
    ///

    pub async fn calc_fees(
        &self,
        auth_account: &AuthAccount,
        messages: &[Message],
    ) -> Result<StdFee, TerraRustAPIError> {
        match &self.gas_options {
            None => Err(TerraRustAPIError::NoGasOpts),

            Some(gas) => {
                match &gas.fees {
                    Some(f) => {
                        let fee_coin: Coin = Coin::create(&f.denom, f.amount);
                        Ok(StdFee::create(vec![fee_coin], gas.gas.unwrap_or(0)))
                    }

                    None => {
                        let fee: StdFee = match &gas.estimate_gas {
                            true => {
                                let default_gas_coin = Coin::create("ukrw", dec!(1.0));
                                let gas_coin = match &gas.gas_price {
                                    Some(c) => c,
                                    None => &default_gas_coin,
                                };
                                let res: LCDResult<TxFeeResult> = self
                                    .tx()
                                    .estimate_fee(
                                        &auth_account.address,
                                        messages,
                                        gas.gas_adjustment.unwrap_or(1.0),
                                        &[gas_coin],
                                    )
                                    .await?;
                                //  let gas_amount = gas.gas_adjustment.unwrap_or(1.0) * res.result.gas as f64;
                                let mut fees: Vec<Coin> = vec![];
                                for fee in res.result.fee.amount {
                                    fees.push(Coin::create(&fee.denom, fee.amount))
                                }
                                StdFee::create(fees, res.result.fee.gas as u64)
                            }
                            false => {
                                let mut fees: Vec<Coin> = vec![];
                                match &gas.fees {
                                    Some(fee) => {
                                        fees.push(Coin::create(&fee.denom, fee.amount));
                                    }
                                    None => {}
                                }

                                StdFee::create(fees, gas.gas.unwrap_or(0))
                            }
                        };
                        Ok(fee)
                    }
                }
            }
        }
    }

    /// helper function to generate a 'StdSignMsg' & 'Signature' blocks to be used to broadcast a transaction
    #[allow(clippy::too_many_arguments)]
    fn generate_transaction_to_broadcast_fees<C: Signing + secp256k1::Context>(
        chain_id: &str,
        auth_account: &AuthAccount,
        fee: StdFee,
        secp: &Secp256k1<C>,
        from: &PrivateKey,
        messages: Vec<Message>,
        memo: Option<String>,
    ) -> Result<(StdSignMsg, Vec<StdSignature>), TerraRustAPIError> {
        let account_number = auth_account.account_number;
        let sequence = auth_account.sequence.unwrap_or(0);
        let messages_len = messages.len();
        let std_sign_msg = StdSignMsg {
            chain_id: chain_id.to_string(), //: String::from(self.chain_id),
            account_number,
            sequence,
            fee,
            msgs: messages,
            memo: memo.unwrap_or(format!(
                "PFC-{}/{}",
                NAME.unwrap_or("TERRA-RUST"),
                VERSION.unwrap_or("dev")
            )),
        };
        let js = serde_json::to_string(&std_sign_msg)?;
        if js.len() > 1000 {
            log::debug!(
                "TO SIGN - {} {} {} #messages {}",
                chain_id,
                account_number,
                sequence,
                messages_len
            );
        } else {
            log::debug!("TO SIGN - {}", js);
        }

        // eprintln!("Client.rs:311\n{}", js);
        let sig = from.sign(secp, &js)?;
        let sigs: Vec<StdSignature> = vec![sig];

        Ok((std_sign_msg, sigs))
    }

    /// helper function to generate a 'StdSignMsg' & 'Signature' blocks to be used to broadcast a transaction
    /// This version calculates fees, and obtains account# and sequence# as well
    pub async fn generate_transaction_to_broadcast<C: secp256k1::Signing + secp256k1::Context>(
        &self,
        secp: &Secp256k1<C>,
        from: &PrivateKey,
        messages: Vec<Message>,
        memo: Option<String>,
    ) -> Result<(StdSignMsg, Vec<StdSignature>), TerraRustAPIError> {
        let from_public = from.public_key(secp);
        let from_account = from_public.account()?;
        let auth = self.auth().account(&from_account, None).await?;
        let fees = self.calc_fees(&auth.result.value, &messages).await?;
        Terra::generate_transaction_to_broadcast_fees(
            &self.chain_id,
            &auth.result.value,
            fees,
            secp,
            from,
            messages,
            memo,
        )
    }
    /// helper: sign & submit the transaction sync
    pub async fn submit_transaction_sync<C: Signing + secp256k1::Context>(
        &self,
        secp: &Secp256k1<C>,
        from: &PrivateKey,
        messages: Vec<Message>,
        memo: Option<String>,
    ) -> Result<TXResultSync, TerraRustAPIError> {
        let (std_sign_msg, sigs) = self
            .generate_transaction_to_broadcast(secp, from, messages, memo)
            .await?;
        let resp = self.tx().broadcast_sync(&std_sign_msg, &sigs).await?;

        match resp.code {
            Some(code) => Err(TxResultError(code, resp.txhash, resp.raw_log)),
            None => Ok(resp),
        }
    }
    /// helper: sign & submit the transaction async
    pub async fn submit_transaction_async<C: Signing + secp256k1::Context>(
        &self,
        secp: &Secp256k1<C>,
        from: &PrivateKey,
        messages: Vec<Message>,
        memo: Option<String>,
    ) -> Result<TXResultAsync, TerraRustAPIError> {
        let (std_sign_msg, sigs) = self
            .generate_transaction_to_broadcast(secp, from, messages, memo)
            .await?;
        let resp = self.tx().broadcast_async(&std_sign_msg, &sigs).await?;
        Ok(resp)
    }

    /// fetch the address book for the production network
    pub async fn production_address_book() -> Result<AddressBook, TerraRustAPIError> {
        Self::address_book(NETWORK_PROD_ADDRESS_BOOK).await
    }
    /// fetch the address book for the testnet network
    pub async fn testnet_address_book() -> Result<AddressBook, TerraRustAPIError> {
        Self::address_book(NETWORK_TEST_ADDRESS_BOOK).await
    }
    /// fetch a address book json structure
    pub async fn address_book(addr_url: &str) -> Result<AddressBook, TerraRustAPIError> {
        if let Some(file_name) = addr_url.strip_prefix("file://") {
            let file = File::open(file_name).unwrap();
            let add: AddressBook = serde_json::from_reader(file)?;
            Ok(add)
        } else {
            let client = reqwest::Client::new();

            let req = client.get(addr_url).headers(Self::construct_headers());
            Ok(Self::resp::<AddressBook>(addr_url, req).await?)
        }
    }
}
#[cfg(test)]
mod tst {
    use super::*;
    //use crate::client::auth::Auth;
    use crate::core_types::{Coin, StdTx};
    use crate::messages::MsgSend;
    use crate::{PrivateKey, Terra};
    use bitcoin::secp256k1::Secp256k1;

    #[test]
    pub fn test_send() -> Result<(), TerraRustAPIError> {
        let str_1 = "island relax shop such yellow opinion find know caught erode blue dolphin behind coach tattoo light focus snake common size analyst imitate employ walnut";
        let secp = Secp256k1::new();
        let pk = PrivateKey::from_words(&secp, str_1, 0, 0)?;
        let pub_k = pk.public_key(&secp);
        let from_address = pub_k.account()?;
        assert_eq!(from_address, "terra1n3g37dsdlv7ryqftlkef8mhgqj4ny7p8v78lg7");

        let send = MsgSend::create_single(
            from_address,
            "terra1usws7c2c6cs7nuc8vma9qzaky5pkgvm2uag6rh".into(),
            Coin::parse("100000uluna")?.unwrap(),
        )?;
        let json = serde_json::to_string(&send)?;
        let json_eq = r#"{"type":"bank/MsgSend","value":{"amount":[{"amount":"100000","denom":"uluna"}],"from_address":"terra1n3g37dsdlv7ryqftlkef8mhgqj4ny7p8v78lg7","to_address":"terra1usws7c2c6cs7nuc8vma9qzaky5pkgvm2uag6rh"}}"#;

        assert_eq!(json, json_eq);
        let std_fee = StdFee::create_single(Coin::parse("50000uluna")?.unwrap(), 90000);

        let messages: Vec<Message> = vec![send];
        let auth_account = AuthAccount {
            address: "terra1n3g37dsdlv7ryqftlkef8mhgqj4ny7p8v78lg7".to_string(),
            public_key: None,
            account_number: 43045,
            sequence: Some(3),
        };
        let (sign_message, signatures) = Terra::generate_transaction_to_broadcast_fees(
            "tequila-0004".into(),
            &auth_account,
            std_fee,
            &secp,
            &pk,
            messages,
            Some("PFC-terra-rust/0.1.5".into()),
        )?;
        let json_sign_message = serde_json::to_string(&sign_message)?;
        let json_sign_message_eq = r#"{"account_number":"43045","chain_id":"tequila-0004","fee":{"amount":[{"amount":"50000","denom":"uluna"}],"gas":"90000"},"memo":"PFC-terra-rust/0.1.5","msgs":[{"type":"bank/MsgSend","value":{"amount":[{"amount":"100000","denom":"uluna"}],"from_address":"terra1n3g37dsdlv7ryqftlkef8mhgqj4ny7p8v78lg7","to_address":"terra1usws7c2c6cs7nuc8vma9qzaky5pkgvm2uag6rh"}}],"sequence":"3"}"#;
        assert_eq!(json_sign_message, json_sign_message_eq);
        let json_sig = serde_json::to_string(&signatures)?;
        let json_sig_eq = r#"[{"signature":"f1wYTzbSyAYqN2tGR0A4PGmfyNYBUExpuoU7UOiBDpNoRlChF/BMtE7h6pdgbpu/V7jNzitu1Eb0fO35dxVkWA==","pub_key":{"type":"tendermint/PubKeySecp256k1","value":"AiMzHaA2bvnDXfHzkjMM+vkSE/p0ymBtAFKUnUtQAeXe"}}]"#;
        assert_eq!(json_sig, json_sig_eq);
        let std_tx: StdTx = StdTx::from_StdSignMsg(&sign_message, &signatures, "sync");
        let js_sig = serde_json::to_string(&std_tx)?;
        let js_sig_eq = r#"{"tx":{"msg":[{"type":"bank/MsgSend","value":{"amount":[{"amount":"100000","denom":"uluna"}],"from_address":"terra1n3g37dsdlv7ryqftlkef8mhgqj4ny7p8v78lg7","to_address":"terra1usws7c2c6cs7nuc8vma9qzaky5pkgvm2uag6rh"}}],"fee":{"amount":[{"amount":"50000","denom":"uluna"}],"gas":"90000"},"signatures":[{"signature":"f1wYTzbSyAYqN2tGR0A4PGmfyNYBUExpuoU7UOiBDpNoRlChF/BMtE7h6pdgbpu/V7jNzitu1Eb0fO35dxVkWA==","pub_key":{"type":"tendermint/PubKeySecp256k1","value":"AiMzHaA2bvnDXfHzkjMM+vkSE/p0ymBtAFKUnUtQAeXe"}}],"memo":"PFC-terra-rust/0.1.5"},"mode":"sync"}"#;
        assert_eq!(js_sig, js_sig_eq);
        Ok(())
    }

    #[test]
    pub fn test_wasm() -> Result<(), TerraRustAPIError> {
        let key_words = "sell raven long age tooth still predict idea quit march gasp bamboo hurdle problem voyage east tiger divide machine brain hole tiger find smooth";
        let secp = Secp256k1::new();
        let private = PrivateKey::from_words(&secp, key_words, 0, 0)?;
        let public_key = private.public_key(&secp);
        let account = public_key.account()?;
        assert_eq!(account, "terra1vr0e7kylhu9am44v0s3gwkccmz7k3naxysrwew");
        //  let gas = GasOptions::create_with_fees("70000uluna", 200000)?;
        //   let terra =
        //       Terra::lcd_client("https://tequila-lcd.terra.dev", "tequila-0004", &gas, None).await?;
        /*
        TODO upgrade test to new version/bombay
        let msg = MsgExecuteContract::create_from_b64(
            &account,
            "terra16ckeuu7c6ggu52a8se005mg5c0kd2kmuun63cu",
            "eyJjYXN0X3ZvdGUiOnsicG9sbF9pZCI6NDQsInZvdGUiOiJ5ZXMiLCJhbW91bnQiOiIxMDAwMDAwIn19",
            &vec![],
        );

        let json = serde_json::to_string(&msg)?;
        let json_eq = r#"{"type":"wasm/MsgExecuteContract","value":{"coins":[],"contract":"terra16ckeuu7c6ggu52a8se005mg5c0kd2kmuun63cu","execute_msg":"eyJjYXN0X3ZvdGUiOnsicG9sbF9pZCI6NDQsInZvdGUiOiJ5ZXMiLCJhbW91bnQiOiIxMDAwMDAwIn19","sender":"terra1vr0e7kylhu9am44v0s3gwkccmz7k3naxysrwew"}}"#;

        assert_eq!(json, json_eq);
        let std_fee = StdFee::create_single(Coin::parse("70000uluna")?.unwrap(), 200000);
        let auth_account = AuthAccount {
            address: "terra1vr0e7kylhu9am44v0s3gwkccmz7k3naxysrwew".to_string(),
            public_key: None,
            account_number: 49411,
            sequence: Some(0),
        };
        let messages: Vec<Message> = vec![msg];
        let (sign_message, signatures) = Terra::generate_transaction_to_broadcast_fees(
            "tequila-0004".into(),
            &auth_account,
            std_fee,
            &secp,
            &private,
            &messages,
            Some("PFC-terra-rust-anchor/0.1.1".into()),
        )?;
        let json_sign_message = serde_json::to_string(&sign_message)?;
        let json_sign_message_eq = r#"{"account_number":"49411","chain_id":"tequila-0004","fee":{"amount":[{"amount":"70000","denom":"uluna"}],"gas":"200000"},"memo":"PFC-terra-rust-anchor/0.1.1","msgs":[{"type":"wasm/MsgExecuteContract","value":{"coins":[],"contract":"terra16ckeuu7c6ggu52a8se005mg5c0kd2kmuun63cu","execute_msg":"eyJjYXN0X3ZvdGUiOnsicG9sbF9pZCI6NDQsInZvdGUiOiJ5ZXMiLCJhbW91bnQiOiIxMDAwMDAwIn19","sender":"terra1vr0e7kylhu9am44v0s3gwkccmz7k3naxysrwew"}}],"sequence":"0"}"#;
        assert_eq!(json_sign_message, json_sign_message_eq);
        let json_sig = serde_json::to_string(&signatures)?;
        let json_sig_eq = r#"[{"signature":"pCkd+nBaz1U3DYw0oY2Arxqc+3jI8QRdaXtYbIle9uh60POxvcUHVk2aN7VklgvnPKF7XGIF04U0sxpq/05Vqg==","pub_key":{"type":"tendermint/PubKeySecp256k1","value":"A3K4ruHQP1yY4dkCp41Djnx6z7KfMjDcvkIB93L3Po9C"}}]"#;
        assert_eq!(json_sig, json_sig_eq);
        let std_tx: StdTx = StdTx::from_StdSignMsg(&sign_message, &signatures, "sync");
        let js_sig = serde_json::to_string(&std_tx)?;
        let js_sig_eq = r#"{"tx":{"msg":[{"type":"wasm/MsgExecuteContract","value":{"coins":[],"contract":"terra16ckeuu7c6ggu52a8se005mg5c0kd2kmuun63cu","execute_msg":"eyJjYXN0X3ZvdGUiOnsicG9sbF9pZCI6NDQsInZvdGUiOiJ5ZXMiLCJhbW91bnQiOiIxMDAwMDAwIn19","sender":"terra1vr0e7kylhu9am44v0s3gwkccmz7k3naxysrwew"}}],"fee":{"amount":[{"amount":"70000","denom":"uluna"}],"gas":"200000"},"signatures":[{"signature":"pCkd+nBaz1U3DYw0oY2Arxqc+3jI8QRdaXtYbIle9uh60POxvcUHVk2aN7VklgvnPKF7XGIF04U0sxpq/05Vqg==","pub_key":{"type":"tendermint/PubKeySecp256k1","value":"A3K4ruHQP1yY4dkCp41Djnx6z7KfMjDcvkIB93L3Po9C"}}],"memo":"PFC-terra-rust-anchor/0.1.1"},"mode":"sync"}"#;
        assert_eq!(js_sig, js_sig_eq);

         */
        Ok(())
    }

    #[tokio::test]
    pub async fn test_address_book() -> Result<(), TerraRustAPIError> {
        let prod = Terra::production_address_book().await?;
        assert!(prod.addrs.len() > 0);
        let test = Terra::testnet_address_book().await?;
        assert!(test.addrs.len() > 0);
        let file_version = Terra::address_book("file://resources/addressbook.json").await?;
        assert_eq!(file_version.key, "775cf30a073ca5e97fb07a00");
        assert!(file_version.addrs.len() > 1);
        assert_eq!(
            file_version.addrs[0].addr.id,
            "ebca6b5d3cc2da9dfdfe4b1c045043fce686f143"
        );

        Ok(())
    }
}
