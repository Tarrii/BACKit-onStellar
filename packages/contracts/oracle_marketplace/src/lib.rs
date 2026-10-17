#![no_std]

mod errors;
mod events;
mod storage;
mod types;

#[cfg(test)]
mod test;

pub use types::{MarketplaceConfig, OracleProvider, OracleRating, SlashProof};

use errors::OracleMarketplaceError;
use events::{emit_oracle_slashed, emit_oracle_deregistered, emit_oracle_rated, emit_oracle_registered, emit_oracle_selected};
use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env, Map, Vec};
use storage::*;

#[contract]
pub struct OracleMarketplace;

#[contractimpl]
impl OracleMarketplace {
    pub fn initialize(
        env: Env,
        admin: Address,
        cooldown_secs: u64,
        default_fee_bps: u32,
    ) -> Result<(), OracleMarketplaceError> {
        if get_config(&env).is_some() {
            return Err(OracleMarketplaceError::AlreadyInitialized);
        }
        admin.require_auth();

        let config = MarketplaceConfig {
            admin,
            cooldown_secs,
            default_fee_bps,
        };
        set_config(&env, &config);
        Ok(())
    }

    pub fn register_oracle(
        env: Env,
        provider: Address,
        pubkey: BytesN<32>,
        fee_bps: u32,
        min_stake: i128,
    ) -> Result<(), OracleMarketplaceError> {
        provider.require_auth();

        if get_oracle(&env, &pubkey).is_some() {
            return Err(OracleMarketplaceError::OracleAlreadyRegistered);
        }
        if fee_bps > 10000 {
            return Err(OracleMarketplaceError::InvalidFee);
        }

        let oracle = OracleProvider {
            pubkey: pubkey.clone(),
            address: provider.clone(),
            fee_bps,
            min_stake,
            staked_amount: min_stake,
            total_resolved: 0,
            total_disputes: 0,
            is_active: true,
            registered_at: env.ledger().timestamp(),
            deregister_after: None,
        };

        // When a stake token is configured, the stake is actually escrowed.
        if let Some(t) = get_stake_token(&env) {
            if min_stake > 0 {
                token::Client::new(&env, &t).transfer(
                    &provider,
                    &env.current_contract_address(),
                    &min_stake,
                );
            }
        }

        set_oracle(&env, &pubkey, &oracle);
        add_to_oracle_list(&env, &pubkey);
        emit_oracle_registered(&env, &provider, &pubkey, fee_bps);

        Ok(())
    }

    pub fn deregister_oracle(
        env: Env,
        provider: Address,
        pubkey: BytesN<32>,
    ) -> Result<(), OracleMarketplaceError> {
        provider.require_auth();
        let config = get_config(&env).ok_or(OracleMarketplaceError::NotInitialized)?;
        let oracle = get_oracle(&env, &pubkey).ok_or(OracleMarketplaceError::OracleNotFound)?;

        if !oracle.is_active {
            return Err(OracleMarketplaceError::OracleNotActive);
        }

        if let Some(deregister_after) = oracle.deregister_after {
            if env.ledger().timestamp() < deregister_after {
                return Err(OracleMarketplaceError::CooldownActive);
            }
        }

        let mut updated = oracle;
        updated.is_active = false;
        updated.deregister_after = Some(env.ledger().timestamp() + config.cooldown_secs);
        set_oracle(&env, &pubkey, &updated);
        remove_from_oracle_list(&env, &pubkey);

        emit_oracle_deregistered(&env, &provider, &pubkey);
        Ok(())
    }

    /// Set the token oracle stakes are escrowed and slashed in (admin only).
    pub fn set_stake_token(env: Env, token: Address) -> Result<(), OracleMarketplaceError> {
        let config = get_config(&env).ok_or(OracleMarketplaceError::NotInitialized)?;
        config.admin.require_auth();
        storage::set_stake_token(&env, &token);
        Ok(())
    }

    /// Slash an oracle proven to have double-signed (governance/admin only).
    /// `proof` holds two different reports for one call, both validly signed
    /// by the oracle's key (a bad signature traps). 50% of the stake goes
    /// to `recipient` (the dispute reporter), 50% is burned, and the oracle
    /// is permanently deactivated.
    pub fn slash_oracle(
        env: Env,
        pubkey: BytesN<32>,
        proof: SlashProof,
        recipient: Address,
    ) -> Result<(), OracleMarketplaceError> {
        let config = get_config(&env).ok_or(OracleMarketplaceError::NotInitialized)?;
        config.admin.require_auth();
        let mut oracle = get_oracle(&env, &pubkey).ok_or(OracleMarketplaceError::OracleNotFound)?;
        if oracle.staked_amount <= 0 {
            return Err(OracleMarketplaceError::NothingToSlash);
        }
        let stake_token = get_stake_token(&env).ok_or(OracleMarketplaceError::StakeTokenNotSet)?;

        if proof.outcome_a == proof.outcome_b
            && proof.price_a == proof.price_b
            && proof.timestamp_a == proof.timestamp_b
        {
            return Err(OracleMarketplaceError::InvalidSlashProof);
        }
        let ma = backit_shared::build_message(&env, proof.call_id, proof.outcome_a, proof.price_a, proof.timestamp_a);
        let mb = backit_shared::build_message(&env, proof.call_id, proof.outcome_b, proof.price_b, proof.timestamp_b);
        env.crypto().ed25519_verify(&pubkey, &ma, &proof.signature_a);
        env.crypto().ed25519_verify(&pubkey, &mb, &proof.signature_b);

        let stake = oracle.staked_amount;
        let bounty = stake / 2;
        let burned = stake - bounty;
        let me = env.current_contract_address();
        let t = token::Client::new(&env, &stake_token);
        if bounty > 0 {
            t.transfer(&me, &recipient, &bounty);
        }
        t.burn(&me, &burned);

        oracle.staked_amount = 0;
        oracle.is_active = false;
        set_oracle(&env, &pubkey, &oracle);
        remove_from_oracle_list(&env, &pubkey);

        emit_oracle_slashed(&env, &pubkey, &recipient, bounty, burned);
        Ok(())
    }

    pub fn get_available_oracles(env: Env) -> Vec<OracleProvider> {
        let list = get_oracle_list(&env);
        let mut result = Vec::new(&env);
        for i in 0..list.len() {
            let pubkey = list.get(i).unwrap();
            if let Some(oracle) = get_oracle(&env, &pubkey) {
                if oracle.is_active {
                    result.push_back(oracle);
                }
            }
        }
        result
    }

    pub fn select_oracle_for_call(
        env: Env,
        call_id: u64,
        oracle_pubkey: BytesN<32>,
    ) -> Result<(), OracleMarketplaceError> {
        let oracle = get_oracle(&env, &oracle_pubkey).ok_or(OracleMarketplaceError::OracleNotFound)?;
        if !oracle.is_active {
            return Err(OracleMarketplaceError::OracleNotActive);
        }

        set_call_oracle(&env, call_id, &oracle_pubkey);
        emit_oracle_selected(&env, call_id, &oracle_pubkey);
        Ok(())
    }

    pub fn get_call_oracle(env: Env, call_id: u64) -> Option<BytesN<32>> {
        get_call_oracle(&env, call_id)
    }

    pub fn rate_oracle(
        env: Env,
        user: Address,
        oracle_pubkey: BytesN<32>,
        satisfied: bool,
    ) -> Result<(), OracleMarketplaceError> {
        user.require_auth();
        let _oracle = get_oracle(&env, &oracle_pubkey).ok_or(OracleMarketplaceError::OracleNotFound)?;

        let mut ratings = get_oracle_ratings(&env, &oracle_pubkey);
        if ratings.contains_key(user.clone()) {
            return Err(OracleMarketplaceError::AlreadyRated);
        }

        ratings.set(user.clone(), satisfied);
        set_oracle_ratings(&env, &oracle_pubkey, &ratings);

        emit_oracle_rated(&env, &oracle_pubkey, &user, satisfied);
        Ok(())
    }

    pub fn get_oracle_metrics(
        env: Env,
        oracle_pubkey: BytesN<32>,
    ) -> Result<(u64, u64), OracleMarketplaceError> {
        let oracle = get_oracle(&env, &oracle_pubkey).ok_or(OracleMarketplaceError::OracleNotFound)?;
        Ok((oracle.total_resolved, oracle.total_disputes))
    }

    pub fn get_config_view(env: Env) -> Result<MarketplaceConfig, OracleMarketplaceError> {
        get_config(&env).ok_or(OracleMarketplaceError::NotInitialized)
    }
}
