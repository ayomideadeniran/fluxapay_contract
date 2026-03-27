# Deployment and Operations Guide

This document records the deployed contract addresses, network configurations, and operational procedures for the FluxaPay protocol on the Stellar network.

## 📋 Table of Contents

- [Contract Registry](#-contract-registry)
- [Network Configuration](#-network-configuration)
- [Deployment Process](#-deployment-process)
- [Pre-Deployment Checklist](#-pre-deployment-checklist)
- [Post-Deployment Verification](#-post-deployment-verification)
- [Upgrade Process](#-upgrade-process)
- [Admin Key Rotation](#-admin-key-rotation)
- [On-Chain Verification](#-on-chain-verification)
- [Emergency Procedures](#-emergency-procedures)

## 🚀 Contract Registry

| Contract Name    | Network | Contract ID              | Deploy Date  | Deployer Address     |
| ---------------- | ------- | ------------------------ | ------------ | -------------------- |
| PaymentProcessor | Testnet | `<PAYMENT_PROCESSOR_ID>` | `YYYY-MM-DD` | `<DEPLOYER_ADDRESS>` |
| RefundManager    | Testnet | `<REFUND_MANAGER_ID>`    | `YYYY-MM-DD` | `<DEPLOYER_ADDRESS>` |
| MerchantRegistry | Testnet | `<MERCHANT_REGISTRY_ID>` | `YYYY-MM-DD` | `<DEPLOYER_ADDRESS>` |
| FXOracle         | Testnet | `<FX_ORACLE_ID>`         | `YYYY-MM-DD` | `<DEPLOYER_ADDRESS>` |

> [!NOTE]
> For Mainnet addresses, please refer to the secure internal dashboard or contact the operations lead.

## 🛠 Deployment Process

### Automated Deployment via GitHub Actions

FluxaPay uses a multi-stage deployment pipeline with mandatory approval gates for production:

1. **Development** (Automatic on main push)
   - Deploys to testnet automatically
   - No approval required
   - Retention: 30 days

2. **Staging** (Automatic after dev success)
   - Deploys to testnet after dev succeeds
   - Requires successful dev deployment
   - Retention: 90 days

3. **Production** (Manual trigger only)
   - Deploys to mainnet
   - Requires manual workflow dispatch
   - Requires 2 team member approvals (GitHub Environment Protection)
   - Must pass staging first
   - Retention: 365 days

### Triggering Production Deployment

1. Navigate to the repository's Actions tab
2. Select "CD" workflow
3. Click "Run workflow"
4. Choose "production" from the target-environment dropdown
5. Click "Run workflow"
6. **Required**: Two team members must approve in GitHub UI before deployment proceeds

## ✅ Pre-Deployment Checklist

**Before deploying to production, verify the following:**

### Code Quality & Testing
- [ ] All CI tests passing (check GitHub Actions status)
- [ ] Integration tests passed locally
- [ ] Testnet deployment successful (staging environment)
- [ ] No breaking changes or proper migration path documented

### Security & Audit
- [ ] Smart contract audit completed by reputable firm
- [ ] All critical/high severity issues resolved
- [ ] Security review of recent changes completed
- [ ] No known vulnerabilities in dependencies (run `cargo audit`)

### Documentation
- [ ] CHANGELOG.md updated with latest changes
- [ ] API documentation current
- [ ] Migration guide prepared if needed
- [ ] Runbook updated for new features

### Team Review
- [ ] Code reviewed and approved by at least 2 team members
- [ ] Product owner sign-off (if feature deployment)
- [ ] Operations team notified of deployment window

### Environment Setup
- [ ] GitHub Environment protection rules configured (require 2 reviewers)
- [ ] Production secrets properly configured:
  - `STELLAR_SECRET_KEY` - Deployer account
  - `PRODUCTION_CONTRACT_ADDRESS` - Current contract address
- [ ] Backup procedures verified

## 🔍 Post-Deployment Verification

**After deployment completes, perform these verification steps:**

### Immediate Verification (Automated)
The CD pipeline automatically:
- Verifies contract is accessible on mainnet
- Logs contract address for verification
- Runs smoke tests

### Manual Verification Steps

1. **Verify Contract Deployment**
   ```bash
   # Check contract exists and is accessible
   stellar contract info interface \
     --id <CONTRACT_ID> \
     --network mainnet
   ```

2. **Verify Admin Configuration**
   ```bash
   # Confirm admin address is correct
   stellar contract invoke \
     --id <CONTRACT_ID> \
     --network mainnet \
     -- get_admin
   ```
   Expected: Returns the expected admin address

3. **Verify Merchant Registry**
   ```bash
   # Check a known merchant
   stellar contract invoke \
     --id <MERCHANT_REGISTRY_ID> \
     --network mainnet \
     -- get_merchant --merchant_id <KNOWN_MERCHANT_ADDRESS>
   ```

4. **Health Check**
   ```bash
   # Verify contract responds correctly
   stellar contract info interface \
     --id <CONTRACT_ID> \
     --network mainnet
   ```

5. **Monitor Initial Transactions**
   - Watch first few transactions on Stellar Expert explorer
   - Verify event logs are correct
   - Check for any error patterns

### Rollback Criteria
Roll back immediately if:
- Contract fails to respond
- Admin configuration incorrect
- Critical functionality broken
- Security vulnerability discovered

## 🛠 Upgrade Process

### Stellar Testnet

- **Horizon URL**: `https://horizon-testnet.stellar.org`
- **Network Passphrase**: `Test SDF Network ; September 2015`
- **RPC URL**: `https://soroban-testnet.stellar.org`

### Stellar Mainnet

- **Horizon URL**: `https://horizon.stellar.org`
- **Network Passphrase**: `Public Global Stellar Network ; September 2015`
- **RPC URL**: `https://soroban-rpc.stellar.org`

## 🚨 Emergency Procedures

### Pausing Operations

If a critical vulnerability or issue is discovered:

1. **Immediate Action**
   - Notify all team members via emergency channel
   - Document the issue and timestamp
   - Halt any pending deployments

2. **Contract-Level Response**
   - If admin controls exist, consider pausing critical functions
   - Monitor ongoing transactions
   - Prepare communication for users if needed

3. **Recovery Steps**
   - Assess severity and scope
   - Develop fix or mitigation
   - Follow expedited review process
   - Deploy fix with enhanced verification

### Contact Information

- **Security Team**: [security@fluxapay.com](mailto:security@fluxapay.com)
- **Operations Lead**: [operations@fluxapay.com](mailto:operations@fluxapay.com)
- **Emergency Channel**: #fluxapay-emergency (Slack)

## 🌐 Network Configuration

The FluxaPay contracts follow the standard Soroban upgrade pattern. Upgrading a contract requires administrative authorization.

### Step 1: Upload New WASM

First, install the new WASM code on the network without deploying it to an instance.

```bash
stellar contract install --wasm target/wasm32-unknown-unknown/release/fluxapay.wasm --network testnet --source <ADMIN_SECRET>
```

Take note of the returned `Wasm Hash`.

### Step 2: Invoke Upgrade

Invoke the `upgrade` function (if implemented) or use the `stellar-cli` to update the contract instance's executable.

> [!IMPORTANT]
> If a custom `upgrade` function is not present in the contract logic, a logic update or a redeployment/migration may be required depending on the specific contract state management.

```bash
# Example if using a custom upgrade function (recommended for state migration)
stellar contract invoke --id <CONTRACT_ID> --network testnet --source <ADMIN_SECRET> -- upgrade --new_wasm_hash <WASM_HASH>
```

## 🔑 Admin Key Rotation

Administrative roles for `PaymentProcessor` and `RefundManager` are managed via the `AccessControl` module.

### Rotating the Admin Role

To transfer administrative control to a new address:

```bash
stellar contract invoke --id <CONTRACT_ID> --network testnet --source <CURRENT_ADMIN_SECRET> \
  -- transfer_admin --current_admin <CURRENT_ADMIN_ADDRESS> --new_admin <NEW_ADMIN_ADDRESS>
```

> [!WARNING]
> Ensure the new admin address is correct and accessible before performing this operation. Administrative control can only be transferred, not recovered without the current admin's signature.

## ✅ On-Chain Verification

To verify the current state and administrative configuration of a deployed contract:

### Verify Admin

```bash
stellar contract invoke --id <CONTRACT_ID> --network testnet -- get_admin
```

### Verify Registry Info

```bash
# Verify a merchant in the registry
stellar contract invoke --id <MERCHANT_REGISTRY_ID> --network testnet -- get_merchant --merchant_id <MERCHANT_ADDRESS>
```

### Health Check (Simulation)

```bash
# Check if the contract is responsive
stellar contract info interface --id <CONTRACT_ID> --network testnet
```
