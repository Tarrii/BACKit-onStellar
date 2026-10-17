use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum GasStationError {
    /// A function requiring initialization was called before `initialize`.
    NotInitialized = 1,
    /// `initialize` was called on an already-initialized contract.
    AlreadyInitialized = 2,
    /// The caller-supplied admin address does not match the stored admin.
    Unauthorized = 3,
    /// `winning_cut_bps` exceeds 10 000 (100%).
    InvalidWinningCutBps = 4,
    /// `max_gas_xlm` (or an effective-stake gas estimate) is not positive,
    /// or would make the effective stake negative.
    InvalidGasAmount = 5,
    /// `user` has no active sponsorship registered with this gas station.
    UserNotSponsored = 6,
    /// This `call_id` has already had its sponsored payout processed.
    CallAlreadyProcessed = 7,
    /// `staker_winning_stake` is 0 or negative; there is nothing to claim.
    InvalidWinningStake = 8,
    /// An arithmetic operation overflowed; the transaction is reverted.
    Overflow = 9,
    /// `refill_gas_pool` was called with a non-positive amount.
    InvalidRefillAmount = 10,
    /// `user` has already used all `MAX_SPONSORED_OPS` sponsored operations.
    SponsorshipLimitReached = 11,
    /// The gas pool cannot cover the per-operation sponsorship fee.
    InsufficientReserve = 12,
}
