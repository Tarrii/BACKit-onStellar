#![cfg(test)]

extern crate std;

use crate::{
    types::{MarketplaceConfig, OracleProvider},
    OracleMarketplace, OracleMarketplaceClient,
};
use soroban_sdk::{
    testutils::Address as AddressTest,
    Address, BytesN, Env,
};

#[test]
fn initialize_marketplace() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);

    let contract_id = env.register(OracleMarketplace, ());
    let client = OracleMarketplaceClient::new(&env, &contract_id);
    client.initialize(&admin, &3600u64, &100u32);

    let config = client.get_config_view();
    assert_eq!(config.admin, admin);
    assert_eq!(config.cooldown_secs, 3600);
    assert_eq!(config.default_fee_bps, 100);
}

#[test]
fn register_and_deregister_oracle() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);

    let contract_id = env.register(OracleMarketplace, ());
    let client = OracleMarketplaceClient::new(&env, &contract_id);
    client.initialize(&admin, &3600u64, &100u32);

    client.register_oracle(&provider, &pubkey, &100u32, &0i128);

    let oracles = client.get_available_oracles();
    assert_eq!(oracles.len(), 1);

    client.deregister_oracle(&provider, &pubkey);

    let oracles = client.get_available_oracles();
    assert_eq!(oracles.len(), 0);
}

#[test]
fn select_oracle_for_call_and_rate() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let user = Address::generate(&env);
    let pubkey = BytesN::from_array(&env, &[1u8; 32]);

    let contract_id = env.register(OracleMarketplace, ());
    let client = OracleMarketplaceClient::new(&env, &contract_id);
    client.initialize(&admin, &3600u64, &100u32);

    client.register_oracle(&provider, &pubkey, &100u32, &0i128);
    client.select_oracle_for_call(&42u64, &pubkey);

    let selected = client.get_call_oracle(&42u64);
    assert_eq!(selected, Some(pubkey.clone()));

    client.rate_oracle(&user, &pubkey, &true);

    let metrics = client.get_oracle_metrics(&pubkey);
    assert_eq!(metrics, (0u64, 0u64));
}

// ── SC-028: slashing ─────────────────────────────────────────────────────────

use crate::errors::OracleMarketplaceError;
use crate::types::SlashProof;
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};

fn sign(env: &Env, secret: &[u8; 32], call_id: u64, outcome: u32, price: i128, ts: u64) -> BytesN<64> {
    use ed25519_dalek::{Signer, SigningKey};
    let msg = backit_shared::build_message(env, call_id, outcome, price, ts);
    let mut buf = [0u8; 128];
    let n = msg.len() as usize;
    msg.copy_into_slice(&mut buf[..n]);
    BytesN::from_array(env, &SigningKey::from_bytes(secret).sign(&buf[..n]).to_bytes())
}

struct Slash<'a> {
    env: Env,
    client: OracleMarketplaceClient<'a>,
    token: Address,
    secret: [u8; 32],
    pubkey: BytesN<32>,
}

fn slash_setup<'a>() -> Slash<'a> {
    use ed25519_dalek::SigningKey;
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let provider = Address::generate(&env);
    let id = env.register(OracleMarketplace, ());
    let client = OracleMarketplaceClient::new(&env, &id);
    client.initialize(&admin, &3600u64, &100u32);

    let token = env.register_stellar_asset_contract_v2(admin).address();
    StellarAssetClient::new(&env, &token).mint(&provider, &1_000);
    client.set_stake_token(&token);

    let secret = [7u8; 32];
    let pubkey = BytesN::from_array(&env, &SigningKey::from_bytes(&secret).verifying_key().to_bytes());
    client.register_oracle(&provider, &pubkey, &100u32, &1_000i128);
    Slash { env, client, token, secret, pubkey }
}

fn proof(s: &Slash, outcome_b: u32) -> SlashProof {
    SlashProof {
        call_id: 9,
        outcome_a: 1,
        price_a: 100,
        timestamp_a: 50,
        signature_a: sign(&s.env, &s.secret, 9, 1, 100, 50),
        outcome_b,
        price_b: 100,
        timestamp_b: 50,
        signature_b: sign(&s.env, &s.secret, 9, outcome_b, 100, 50),
    }
}

#[test]
fn double_signing_is_slashed_half_to_reporter_half_burned() {
    let s = slash_setup();
    let reporter = Address::generate(&s.env);
    s.client.slash_oracle(&s.pubkey, &proof(&s, 2), &reporter);

    let t = TokenClient::new(&s.env, &s.token);
    assert_eq!(t.balance(&reporter), 500);
    assert_eq!(t.balance(&s.client.address), 0); // other 500 burned
    assert_eq!(s.client.get_available_oracles().len(), 0);

    // Permanently revoked: cannot be slashed again or re-registered.
    assert_eq!(
        s.client.try_slash_oracle(&s.pubkey, &proof(&s, 2), &reporter),
        Err(Ok(OracleMarketplaceError::NothingToSlash))
    );
    let again = Address::generate(&s.env);
    assert_eq!(
        s.client.try_register_oracle(&again, &s.pubkey, &100u32, &0i128),
        Err(Ok(OracleMarketplaceError::OracleAlreadyRegistered))
    );
}

#[test]
fn identical_reports_are_not_proof_of_fraud() {
    let s = slash_setup();
    let reporter = Address::generate(&s.env);
    assert_eq!(
        s.client.try_slash_oracle(&s.pubkey, &proof(&s, 1), &reporter),
        Err(Ok(OracleMarketplaceError::InvalidSlashProof))
    );
    assert_eq!(s.client.get_available_oracles().len(), 1);
}

#[test]
#[should_panic]
fn a_forged_signature_cannot_slash_an_honest_oracle() {
    let s = slash_setup();
    let mut p = proof(&s, 2);
    p.signature_b = sign(&s.env, &[9u8; 32], 9, 2, 100, 50); // wrong key
    s.client.slash_oracle(&s.pubkey, &p, &Address::generate(&s.env));
}
