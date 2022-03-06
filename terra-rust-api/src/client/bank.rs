use crate::core_types::Coin;
use crate::{LCDResultVec, Terra};

pub struct Bank<'a> {
    terra: &'a Terra,
}
impl Bank<'_> {
    pub fn create(terra: &'_ Terra) -> Bank<'_> {
        Bank { terra }
    }
    pub async fn balances(
        &self,
        account_address: &str,
        height: Option<u64>,
    ) -> anyhow::Result<LCDResultVec<Coin>> {
        let response = self
            .terra
            .send_cmd::<LCDResultVec<Coin>>(
                &format!("/bank/balances/{}", account_address),
                None,
                height,
            )
            .await?;
        Ok(response)
    }
}
