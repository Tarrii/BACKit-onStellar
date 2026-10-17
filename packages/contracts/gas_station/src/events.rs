#![allow(deprecated)]

use soroban_sdk::{Address, Env, Symbol};

pub fn emit_gas_station_initialized(env: &Env, admin: &Address, xlm_token: &Address) {
    env.events().publish(
        ("gas_station", "initialized"),
        (admin.clone(), xlm_token.clone()),
    );
}

pub fn emit_admin_changed(env: &Env, new_admin: &Address) {
    env.events()
        .publish(("gas_station", "admin_changed"), new_admin.clone());
}

pub fn emit_sponsorship_registered(
    env: &Env,
    user: &Address,
    max_gas_xlm: i128,
    winning_cut_bps: u32,
) {
    env.events().publish(
        ("gas_station", "sponsorship_registered"),
        (user.clone(), max_gas_xlm, winning_cut_bps),
    );
}

pub fn emit_sponsorship_revoked(env: &Env, user: &Address) {
    env.events()
        .publish(("gas_station", "sponsorship_revoked"), user.clone());
}

pub fn emit_sponsored_payout_claimed(
    env: &Env,
    call_id: u64,
    user: &Address,
    gross_payout: i128,
    cut: i128,
    user_amount: i128,
) {
    env.events().publish(
        ("gas_station", "sponsored_payout_claimed"),
        (call_id, user.clone(), gross_payout, cut, user_amount),
    );
}

pub fn emit_pool_refilled(env: &Env, amount: i128, new_balance: i128) {
    env.events()
        .publish(("gas_station", "pool_refilled"), (amount, new_balance));
}

pub fn emit_transaction_sponsored(env: &Env, user: &Address, op_type: &Symbol) {
    env.events().publish(
        ("gas_station", "TransactionSponsored"),
        (user.clone(), op_type.clone()),
    );
}
