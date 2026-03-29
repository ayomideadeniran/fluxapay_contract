# Local Testnet Invoke Recipes

This guide shows how to invoke Fluxapay contract functions locally on Stellar testnet using the Stellar CLI.

## Prerequisites

1. **Install Stellar CLI**: [stellar-cli](https://github.com/stellar/rs-soroban-cli)
2. **Set up environment variables**: Copy `.env.example` to `.env` and populate with your testnet values:
   ```bash
   cp .env.example .env
   ```
3. **Generate test keypairs** (if needed):
   ```bash
   stellar keys generate --name test-admin
   stellar keys generate --name test-merchant
   stellar keys generate --name test-customer
   ```
4. **Fund test accounts** on testnet via the [Stellar Friendbot](https://friendbot.stellar.org/)

---

## Setup & Configuration

### Load Environment Variables

```bash
# Load from .env (supported by many shells)
export $(cat .env | grep -v '#' | xargs)

# Or load manually for your shell
source .env        # bash/zsh
set -a; source .env; set +a  # sh
```

### Verify Network Configuration

```bash
# Check testnet connectivity
stellar contract info interface \
  --id $PAYMENT_PROCESSOR_ID \
  --network testnet
```

---

## Core Contract Functions

### 1. Register Merchant

Register a new merchant on the Merchant Registry contract. The merchant must authenticate and provide KYC details.

#### Function Signature

```rust
pub fn register_merchant(
    env: Env,
    merchant_id: Address,           // Merchant's Stellar address
    business_name: String,          // Legal business name
    settlement_currency: String,    // e.g. "USD", "EUR", "NGN"
    payout_address: Option<Address>,// Optional payout wallet address
    bank_account: Option<String>,   // Optional bank account reference
) -> Result<(), MerchantError>
```

#### Invoke Command

```bash
stellar contract invoke \
  --id $MERCHANT_REGISTRY_ID \
  --network testnet \
  --source $TEST_MERCHANT_ADDRESS \
  -- register_merchant \
  --merchant_id $TEST_MERCHANT_ADDRESS \
  --business_name "TechCorp Nigeria Limited" \
  --settlement_currency "NGN" \
  --payout_address $ADMIN_ADDRESS \
  --bank_account "0123456789"
```

#### Expected Output

Success:
```json
{
  "status": "success"
}
```

#### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `MerchantAlreadyExists` | Merchant already registered | Use a different address or verify registration |
| `Unauthorized` | Request not signed by merchant | Ensure `--source` matches `--merchant_id` |

#### Verification

After registration, verify the merchant was created:

```bash
stellar contract invoke \
  --id $MERCHANT_REGISTRY_ID \
  --network testnet \
  -- get_merchant \
  --merchant_id $TEST_MERCHANT_ADDRESS
```

---

### 2. Create Payment

Create a payment charge that a customer must fulfill by sending USDC to the deposit address.

#### Function Signature

```rust
pub fn create_payment(
    env: Env,
    payment_id: String,             // Unique payment identifier
    merchant_id: Address,           // Merchant creating the charge
    amount: i128,                   // Amount in stroops (1 USDC = 10^7 stroops)
    currency: Symbol,               // e.g. "USDC"
    deposit_address: Address,       // Where customer sends funds
    expires_at: u64,                // Unix timestamp when payment expires
    memo: Option<String>,           // Optional memo/invoice reference
    memo_type: Option<String>,      // Optional memo type (e.g. "invoice_id", "order_id")
) -> Result<PaymentCharge, Error>
```

#### Pre-requisites

1. Merchant must be registered (see [Register Merchant](#1-register-merchant))
2. Merchant must be verified by admin (contact operations team for testnet)
3. Deposit address must be a valid Stellar address
4. Expiration timestamp must be in the future

#### Invoke Command

```bash
# Calculate expiration (current time + 1 hour, adjust as needed)
EXPIRES_AT=$(($(date +%s) + 3600))

stellar contract invoke \
  --id $PAYMENT_PROCESSOR_ID \
  --network testnet \
  --source $TEST_MERCHANT_ADDRESS \
  -- create_payment \
  --payment_id "inv_20260329_001" \
  --merchant_id $TEST_MERCHANT_ADDRESS \
  --amount 1000000000 \
  --currency USDC \
  --deposit_address $ADMIN_ADDRESS \
  --expires_at $EXPIRES_AT \
  --memo "Order #12345" \
  --memo_type "order_id"
```

#### Expected Output

Success returns a `PaymentCharge` object:
```json
{
  "payment_id": "inv_20260329_001",
  "merchant_id": "GXXXXXX...",
  "amount": 1000000000,
  "currency": "USDC",
  "status": "Pending",
  "created_at": 1711776000,
  "expires_at": 1711779600,
  "memo": "Order #12345",
  "memo_type": "order_id"
}
```

#### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `Unauthorized` | Merchant not verified or role not granted | Contact admin to verify merchant |
| `InvalidAmount` | Amount ≤ 0 | Specify positive amount in stroops |
| `PaymentAlreadyExists` | Payment ID already used | Use a unique payment ID |
| `ContractPaused` | Contract is paused | Contact admin to unpause |

#### Field Conversion Guide

- **Amount**: Stellar uses stroops (1 USDC = 10^7 stroops)
  - 1 USDC → `10000000` stroops (7 decimal places)
  - 100 USDC → `1000000000` stroops

---

### 3. Get Payment Status

Retrieve the current status of a payment charge.

#### Function Signature

```rust
pub fn get_payment(
    env: Env,
    payment_id: String,  // Payment identifier
) -> Result<PaymentCharge, Error>
```

#### Invoke Command

```bash
stellar contract invoke \
  --id $PAYMENT_PROCESSOR_ID \
  --network testnet \
  -- get_payment \
  --payment_id "inv_20260329_001"
```

#### Expected Output

Returns the full `PaymentCharge` object with current status:
```json
{
  "payment_id": "inv_20260329_001",
  "merchant_id": "GXXXXXX...",
  "amount": 1000000000,
  "currency": "USDC",
  "status": "Confirmed",
  "amount_received": 1000000000,
  "created_at": 1711776000,
  "expires_at": 1711779600,
  "memo": "Order #12345",
  "memo_type": "order_id"
}
```

#### Payment Status Values

| Status | Meaning | Next Step |
|--------|---------|-----------|
| `Pending` | Awaiting payment | Customer sends USDC to deposit address |
| `Confirmed` | Payment received in full | Merchant fulfills order |
| `PartiallyPaid` | Underpayment received | Request additional funds or refund |
| `Overpaid` | Overpayment received | Refund excess or reconcile |
| `Expired` | Payment deadline passed | Issue new payment link |
| `Failed` | Payment verification failed | Troubleshoot or create new charge |

#### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `PaymentNotFound` | Payment ID doesn't exist | Verify payment ID matches created payment |

---

## Advanced Recipes

### Verify Merchant Payments (Enumeration)

List all payments for a specific merchant:

```bash
stellar contract invoke \
  --id $PAYMENT_PROCESSOR_ID \
  --network testnet \
  -- get_merchant_payments \
  --merchant_id $TEST_MERCHANT_ADDRESS
```

### Paginated Merchant Payments

Fetch merchant payments with pagination:

```bash
stellar contract invoke \
  --id $PAYMENT_PROCESSOR_ID \
  --network testnet \
  -- get_merchant_payments_paginated \
  --merchant_id $TEST_MERCHANT_ADDRESS \
  --offset 0 \
  --limit 10
```

### Monitor Payment Events

Monitor payment verification events in real-time:

```bash
# View recent ledger entries for payment events
stellar contract invoke \
  --id $PAYMENT_PROCESSOR_ID \
  --network testnet \
  -- --monitor-events
```

---

## Troubleshooting

### Contract Not Found

```bash
# Error: Contract not found
# Solution: Verify contract ID is deployed and correct
stellar contract info interface \
  --id $PAYMENT_PROCESSOR_ID \
  --network testnet
```

### Invalid Network Passphrase

```bash
# Error: Invalid network passphrase
# Solution: Ensure you're using testnet and correct settings
export STELLAR_NETWORK=testnet
```

### Insufficient Balance

```bash
# Error: Account requires minimum balance
# Solution: Fund account via Friendbot
# https://friendbot.stellar.org/?addr=<YOUR_ADDRESS>
```

### Insufficient Signatures

```bash
# Error: Signature verification failed
# Solution: Ensure --source matches the signer for the operation
# Most operations require authentication from the affected party
```

---

## Tips for Local Development

1. **Use consistent merchants**: Register one test merchant and reuse its ID for multiple payments
2. **Generate future timestamps**: Use `date +%s` and add seconds for realistic expiration times
3. **Batch operations**: Chain multiple invocations in a shell script for integration testing
4. **Save outputs**: Redirect results to JSON files for audit trails:
   ```bash
   stellar contract invoke ... >> payment_log.json
   ```
5. **Validate before submitting**: Always check payment amounts and expiration dates before creating charges

---

## Quick Test Loop

```bash
#!/bin/bash
source .env

# Setup
EXPIRES_AT=$(($(date +%s) + 3600))
PAYMENT_ID="test_$(date +%s)"

# 1. Register merchant (one-time)
echo "Registering merchant..."
stellar contract invoke \
  --id $MERCHANT_REGISTRY_ID \
  --network testnet \
  --source $TEST_MERCHANT_ADDRESS \
  -- register_merchant \
  --merchant_id $TEST_MERCHANT_ADDRESS \
  --business_name "Test Shop" \
  --settlement_currency "USD"

# 2. Create payment
echo "Creating payment..."
stellar contract invoke \
  --id $PAYMENT_PROCESSOR_ID \
  --network testnet \
  --source $TEST_MERCHANT_ADDRESS \
  -- create_payment \
  --payment_id $PAYMENT_ID \
  --merchant_id $TEST_MERCHANT_ADDRESS \
  --amount 1000000000 \
  --currency USDC \
  --deposit_address $ADMIN_ADDRESS \
  --expires_at $EXPIRES_AT

# 3. Verify payment created
echo "Verifying payment status..."
stellar contract invoke \
  --id $PAYMENT_PROCESSOR_ID \
  --network testnet \
  -- get_payment \
  --payment_id $PAYMENT_ID
```

---

## Additional Resources

- [Stellar CLI Documentation](https://github.com/stellar/rs-soroban-cli)
- [Soroban SDK Examples](https://github.com/stellar/rs-soroban-sdk)
- [Stellar Testnet](https://testnet.stellar.org/)
- [Fluxapay API Documentation](../README.md)
- [Deployment Guide](../DEPLOYMENT.md)
