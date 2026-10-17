#![allow(deprecated)]

use soroban_sdk::{Address, BytesN, Env};

pub fn emit_oracle_registered(env: &Env, provider: &Address, pubkey: &BytesN<32>, fee_bps: u32) {
    env.events().publish(
        ("oracle_marketplace", "OracleRegistered"),
        (provider.clone(), pubkey.clone(), fee_bps),
    );
}

pub fn emit_oracle_deregistered(env: &Env, provider: &Address, pubkey: &BytesN<32>) {
    env.events().publish(
        ("oracle_marketplace", "OracleDeregistered"),
        (provider.clone(), pubkey.clone()),
    );
}

pub fn emit_oracle_selected(env: &Env, call_id: u64, provider: &BytesN<32>) {
    env.events().publish(
        ("oracle_marketplace", "OracleSelectedForCall"),
        (call_id, provider.clone()),
    );
}

pub fn emit_oracle_rated(env: &Env, provider: &BytesN<32>, user: &Address, satisfied: bool) {
    env.events().publish(
        ("oracle_marketplace", "OracleRated"),
        (provider.clone(), user.clone(), satisfied),
    );
}

pub fn emit_oracle_slashed(env: &Env, pubkey: &BytesN<32>, recipient: &Address, bounty: i128, burned: i128) {
    env.events().publish(
        ("oracle_marketplace", "OracleSlashed"),
        (pubkey.clone(), recipient.clone(), bounty, burned),
    );
}
