use soroban_sdk::{contract, contracterror, contractimpl, contracttype, token, Address, Env, String, Symbol};

// ─── Data types ───────────────────────────────────────────────────────────────

/// Status of a payment stream.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamStatus {
    /// Stream is actively draining the deposit at `rate_per_second`.
    Active,
    /// Stream was cancelled by the sender; residual deposit already refunded.
    Cancelled,
    /// Deposit was fully drained; stream reached its natural end.
    Exhausted,
}

/// A continuous payment stream from `sender` to `receiver`.
///
/// Tokens flow at `rate_per_second` until either the deposit is exhausted or
/// the stream is cancelled. The sender may call [`decrease_rate_per_second`] at
/// any time to slow the flow; accrued amounts are check-pointed before the new
/// rate takes effect and any surplus deposit is refunded.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentStream {
    /// Unique identifier for this stream.
    pub stream_id: String,
    /// Address that funded the stream (payer).
    pub sender: Address,
    /// Address receiving the streamed tokens.
    pub receiver: Address,
    /// Token contract address (e.g. USDC).
    pub token: Address,
    /// Current flow rate in the smallest token unit per second.
    pub rate_per_second: i128,
    /// Total deposit locked in this contract on behalf of the stream.
    pub remaining_deposit: i128,
    /// Ledger timestamp of the last checkpoint.
    pub last_checkpoint_at: u64,
    /// Cumulative tokens accrued **up to** `last_checkpoint_at`.
    ///
    /// Accrual since the last checkpoint is calculated lazily:
    /// `total_accrued = accrued_at_checkpoint + (now - last_checkpoint_at) * rate_per_second`
    pub accrued_at_checkpoint: i128,
    /// Stream lifecycle state.
    pub status: StreamStatus,
}

/// Storage key for a [`PaymentStream`].
#[contracttype]
pub enum StreamDataKey {
    Stream(String),
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StreamError {
    /// No stream exists with the given ID.
    StreamNotFound = 1,
    /// Caller is not the sender of the stream.
    Unauthorized = 2,
    /// The new rate must be strictly less than the current rate.
    RateNotDecreased = 3,
    /// Rate cannot be zero or negative.
    InvalidRate = 4,
    /// A stream with that ID already exists.
    StreamAlreadyExists = 5,
    /// Deposit must be positive.
    InvalidDeposit = 6,
    /// Stream is not active.
    StreamNotActive = 7,
}

// ─── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct PaymentStreaming;

#[contractimpl]
#[allow(deprecated)] // events::publish — migrate to #[contractevent] in a follow-up
impl PaymentStreaming {
    /// Contract version bump helper.
    pub fn version() -> u32 {
        1
    }

    // ─── Stream creation ──────────────────────────────────────────────────────

    /// Create a new payment stream.
    ///
    /// The caller (sender) transfers `deposit` tokens from their account into
    /// this contract. Streaming begins immediately at `rate_per_second`.
    ///
    /// # Parameters
    /// * `sender`         – Account funding the stream; must sign the transaction.
    /// * `receiver`       – Account that will receive streamed tokens.
    /// * `token`          – Token contract address.
    /// * `rate_per_second`– Tokens per second to stream.
    /// * `deposit`        – Total tokens deposited upfront.
    /// * `stream_id`      – Caller-supplied unique identifier.
    pub fn create_stream(
        env: Env,
        sender: Address,
        receiver: Address,
        token: Address,
        rate_per_second: i128,
        deposit: i128,
        stream_id: String,
    ) -> Result<PaymentStream, StreamError> {
        sender.require_auth();

        if rate_per_second <= 0 {
            return Err(StreamError::InvalidRate);
        }
        if deposit <= 0 {
            return Err(StreamError::InvalidDeposit);
        }
        if env
            .storage()
            .persistent()
            .has(&StreamDataKey::Stream(stream_id.clone()))
        {
            return Err(StreamError::StreamAlreadyExists);
        }

        // Transfer deposit from sender into this contract.
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&sender, &env.current_contract_address(), &deposit);

        let now = env.ledger().timestamp();
        let stream = PaymentStream {
            stream_id: stream_id.clone(),
            sender,
            receiver,
            token,
            rate_per_second,
            remaining_deposit: deposit,
            last_checkpoint_at: now,
            accrued_at_checkpoint: 0,
            status: StreamStatus::Active,
        };

        env.storage()
            .persistent()
            .set(&StreamDataKey::Stream(stream_id.clone()), &stream);

        env.events().publish(
            (
                Symbol::new(&env, "STREAM"),
                Symbol::new(&env, "CREATED"),
                stream_id,
            ),
            (stream.sender.clone(), stream.receiver.clone(), deposit),
        );

        Ok(stream)
    }

    // ─── Rate decrease ────────────────────────────────────────────────────────

    /// Reduce the flow rate of an active stream.
    ///
    /// This function:
    /// 1. **Checkpoints** the `accrued_amount` and current timestamp so that
    ///    earnings up to this moment are locked in before the rate changes.
    /// 2. **Refunds** any portion of the deposit that exceeds the tokens still
    ///    accruing at the new (lower) rate:
    ///    `surplus = remaining_deposit - accrued_since_checkpoint - new_needed`
    ///    where `new_needed` is what the new rate needs to sustain for the same
    ///    remaining seconds the old rate would have run.
    /// 3. **Emits** a `RateDecreased` event.
    ///
    /// # Parameters
    /// * `sender`       – Must be the original stream sender; must sign.
    /// * `stream_id`    – Stream to update.
    /// * `new_rate`     – New rate (must be strictly less than the current rate).
    pub fn decrease_rate_per_second(
        env: Env,
        sender: Address,
        stream_id: String,
        new_rate: i128,
    ) -> Result<(), StreamError> {
        sender.require_auth();

        // ── Load stream ──────────────────────────────────────────────────────
        let mut stream: PaymentStream = env
            .storage()
            .persistent()
            .get(&StreamDataKey::Stream(stream_id.clone()))
            .ok_or(StreamError::StreamNotFound)?;

        // ── Authorization ────────────────────────────────────────────────────
        if stream.sender != sender {
            return Err(StreamError::Unauthorized);
        }

        // ── Validation ───────────────────────────────────────────────────────
        if stream.status != StreamStatus::Active {
            return Err(StreamError::StreamNotActive);
        }
        if new_rate <= 0 {
            return Err(StreamError::InvalidRate);
        }
        if new_rate >= stream.rate_per_second {
            return Err(StreamError::RateNotDecreased);
        }

        let now = env.ledger().timestamp();
        let old_rate = stream.rate_per_second;

        // ── Step 1: Checkpoint accrued_amount ─────────────────────────────────
        // Calculate how many tokens accrued since the last checkpoint.
        let elapsed = now.saturating_sub(stream.last_checkpoint_at);
        let newly_accrued = (elapsed as i128).saturating_mul(old_rate);

        // Clamp so we never accrue more than the remaining deposit.
        let newly_accrued = newly_accrued.min(stream.remaining_deposit - stream.accrued_at_checkpoint);

        stream.accrued_at_checkpoint = stream
            .accrued_at_checkpoint
            .saturating_add(newly_accrued);
        stream.last_checkpoint_at = now;

        // ── Step 2: Calculate surplus and refund ──────────────────────────────
        // Tokens not yet transferred to the receiver that are no longer needed
        // given the lower rate are returned to the sender.
        //
        // remaining_unlocked = remaining_deposit − accrued_at_checkpoint
        //   (i.e. the portion of deposit not yet "earned" by receiver)
        //
        // With the old rate those unlocked tokens would last:
        //   old_seconds_left = remaining_unlocked / old_rate
        //
        // At the new (lower) rate those same seconds need fewer tokens:
        //   new_needed = old_seconds_left * new_rate
        //
        // surplus = remaining_unlocked − new_needed
        let remaining_unlocked = stream
            .remaining_deposit
            .saturating_sub(stream.accrued_at_checkpoint);

        let surplus = if remaining_unlocked > 0 && old_rate > 0 {
            // integer division — gives floor of seconds left at old rate
            let old_seconds_left = remaining_unlocked / old_rate;
            let new_needed = old_seconds_left.saturating_mul(new_rate);
            remaining_unlocked.saturating_sub(new_needed).max(0)
        } else {
            0
        };

        // Apply the new rate.
        stream.rate_per_second = new_rate;

        // Deduct surplus from remaining deposit.
        if surplus > 0 {
            stream.remaining_deposit = stream.remaining_deposit.saturating_sub(surplus);

            // Transfer surplus back to sender.
            let token_client = token::Client::new(&env, &stream.token);
            token_client.transfer(&env.current_contract_address(), &stream.sender, &surplus);
        }

        // ── Persist updated stream ────────────────────────────────────────────
        env.storage()
            .persistent()
            .set(&StreamDataKey::Stream(stream_id.clone()), &stream);

        // ── Step 3: Emit RateDecreased event ─────────────────────────────────
        env.events().publish(
            (
                Symbol::new(&env, "STREAM"),
                Symbol::new(&env, "RATE_DECREASED"),
                stream_id,
            ),
            (sender, old_rate, new_rate, surplus),
        );

        Ok(())
    }

    // ─── Read helpers ─────────────────────────────────────────────────────────

    /// Return the stored stream, or `StreamNotFound`.
    pub fn get_stream(env: Env, stream_id: String) -> Result<PaymentStream, StreamError> {
        env.storage()
            .persistent()
            .get(&StreamDataKey::Stream(stream_id))
            .ok_or(StreamError::StreamNotFound)
    }

    /// Compute the total tokens accrued at the current ledger timestamp.
    ///
    /// This is a **view-only** helper and does not modify state.
    pub fn get_accrued_amount(env: Env, stream_id: String) -> Result<i128, StreamError> {
        let stream: PaymentStream = env
            .storage()
            .persistent()
            .get(&StreamDataKey::Stream(stream_id))
            .ok_or(StreamError::StreamNotFound)?;

        if stream.status != StreamStatus::Active {
            return Ok(stream.accrued_at_checkpoint);
        }

        let now = env.ledger().timestamp();
        let elapsed = now.saturating_sub(stream.last_checkpoint_at);
        let newly_accrued = (elapsed as i128).saturating_mul(stream.rate_per_second);
        let total = stream
            .accrued_at_checkpoint
            .saturating_add(newly_accrued)
            .min(stream.remaining_deposit);

        Ok(total)
    }
}
