use crate::auth_types::AuthAccount;
use crate::errors::TerraRustAPIError;
use crate::staking_types::{Validator, ValidatorDelegation, ValidatorUnbondingDelegation};
use crate::{LCDResult, LCDResultVec, LCDTypeValue, Terra};

pub struct Auth<'a> {
    terra: &'a Terra,
}
impl Auth<'_> {
    pub fn create(terra: &'_ Terra) -> Auth<'_> {
        Auth { terra }
    }
    pub async fn account(
        &self,
        account_address: &str,
        height: Option<u64>,
    ) -> Result<LCDResult<LCDTypeValue<AuthAccount>>, TerraRustAPIError> {
        self.terra
            .send_cmd::<LCDResult<LCDTypeValue<AuthAccount>>>(
                &format!("/auth/accounts/{}", account_address),
                None,
                height,
            )
            .await
    }
    /// all delegations for a given account
    pub async fn validator_delegations(
        &self,
        account_address: &str,
        height: Option<u64>,
    ) -> Result<LCDResultVec<ValidatorDelegation>, TerraRustAPIError> {
        self.terra
            .send_cmd::<LCDResultVec<ValidatorDelegation>>(
                &format!("/staking/delegators/{}/delegations", account_address),
                None,
                height,
            )
            .await
    }
    /// all unbonding delegations for a given account
    pub async fn validator_unbonding_delegations(
        &self,
        account_address: &str,
        height: Option<u64>,
    ) -> Result<LCDResult<ValidatorUnbondingDelegation>, TerraRustAPIError> {
        self.terra
            .send_cmd::<LCDResult<ValidatorUnbondingDelegation>>(
                &format!(
                    "/staking/delegators/{}/unbonding_delegations",
                    account_address,
                ),
                None,
                height,
            )
            .await
    }
    /// all validators for a given account
    pub async fn delegated_validators(
        &self,
        account_address: &str,
        height: Option<u64>,
    ) -> Result<LCDResult<Vec<Validator>>, TerraRustAPIError> {
        self.terra
            .send_cmd::<LCDResult<Vec<Validator>>>(
                &format!("/staking/delegators/{}/validators", account_address),
                None,
                height,
            )
            .await
    }
}
