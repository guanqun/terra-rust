use crate::client::core_types::Coin;

use crate::{LCDResult, Message, Terra};
use rust_decimal::Decimal;

use crate::messages::market::MsgSwap;
use futures::future::join_all;

/// Market functions. mainly around swapping tokens
pub struct Market<'a> {
    terra: &'a Terra,
}
impl Market<'_> {
    pub fn create(terra: &'_ Terra) -> Market<'_> {
        Market { terra }
    }
    /// obtain how much a coin is worth in a secondary coin
    pub async fn swap(
        &self,
        offer: &Coin,
        ask_denom: &str,
        height: Option<u64>,
    ) -> anyhow::Result<LCDResult<Coin>> {
        let response = self
            .terra
            .send_cmd::<LCDResult<Coin>>(
                "/market/swap",
                Some(&format!("?offer_coin={}&ask_denom={}", offer, ask_denom)),
                height,
            )
            .await?;
        Ok(response)
    }
    /// generate a set of transactions to swap a account's tokens into another, as long as they are above a certain threshold
    pub async fn generate_sweep_messages(
        &self,
        from: String,
        to_coin: String,
        threshold: Decimal,
        height: Option<u64>,
    ) -> anyhow::Result<Vec<Message>> {
        let account_balances = self.terra.bank().balances(&from, height).await?;
        let potential_coins = account_balances
            .result
            .into_iter()
            .filter(|c| c.denom != to_coin);
        //.collect::<Vec<Coin>>();
        let into_currency_futures = potential_coins
            .into_iter()
            .map(|c| async {
                let resp = self
                    .terra
                    .market()
                    .swap(&c.clone(), &to_coin, height)
                    .await
                    .map(|f| (c, f.result));
                resp
            })
            .collect::<Vec<_>>();

        let into_currency = join_all(into_currency_futures).await;

        let mut err = None;
        let to_convert = &into_currency
            .into_iter()
            .flat_map(|f| match f {
                Ok(coins) => {
                    if coins.1.amount > threshold {
                        Some(coins)
                    } else {
                        None
                    }
                }
                Err(e) => {
                    eprintln!("Error  {}", e);
                    err = Some(e);
                    None
                }
            })
            .collect::<Vec<_>>();
        match err {
            Some(e) => Err(e),
            None => {
                let mut messages = Vec::new();
                for swap_coins in to_convert {
                    let message =
                        MsgSwap::create(swap_coins.0.clone(), to_coin.clone(), from.clone())?;
                    messages.push(message);
                }
                Ok(messages)
            }
        }
    }
}
