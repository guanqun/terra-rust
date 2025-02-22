use crate::errors::TerraRustAPIError;
use crate::Terra;
use rust_decimal::Decimal;
use std::collections::HashMap;

pub struct FCD<'a> {
    terra: &'a Terra,
    fcd_url: &'a str,
}
impl FCD<'_> {
    pub fn create<'a>(terra: &'a Terra, fcd_url: &'a str) -> FCD<'a> {
        FCD { terra, fcd_url }
    }
    pub async fn gas_prices(&self) -> Result<HashMap<String, Decimal>, TerraRustAPIError> {
        Ok(self
            .terra
            .send_cmd_url::<HashMap<String, Decimal>>(
                self.fcd_url,
                "/v1/txs/gas_prices",
                None,
                None,
            )
            .await?)
    }
    pub async fn fetch_gas_prices(
        client: &reqwest::Client,
        fcd_url: &str,
    ) -> Result<HashMap<String, Decimal>, TerraRustAPIError> {
        Ok(Terra::fetch_url::<HashMap<String, Decimal>>(
            client,
            fcd_url,
            "/v1/txs/gas_prices",
            None,
        )
        .await?)
    }
}
