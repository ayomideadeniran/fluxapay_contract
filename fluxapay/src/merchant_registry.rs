use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String};

#[contract]
pub struct MerchantRegistry;

/// KYC tier for merchants, replacing the binary `verified: bool` field.
/// Allows payment limits and settlement schedules to vary by tier.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KycTier {
    Unverified,
    Basic,
    Full,
    Business,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Merchant {
    pub merchant_id: Address,
    pub business_name: String,
    pub settlement_currency: String,
    /// KYC tier replaces the old `verified: bool` field.
    pub kyc_tier: KycTier,
    pub active: bool,
    pub created_at: u64,
}

#[contracttype]
pub enum DataKey {
    Merchant(Address),
    Admin,
    /// Stores the list of all registered merchants for enumeration
    MerchantList,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    MerchantAlreadyExists = 1,
    MerchantNotFound = 2,
    Unauthorized = 3,
    NotVerified = 4,
    AdminAlreadySet = 5,
}

#[contractimpl]
impl MerchantRegistry {
    /// Initialize the contract with an admin address
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().persistent().has(&DataKey::Admin) {
            return Err(Error::AdminAlreadySet);
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Register a new merchant
    pub fn register_merchant(
        env: Env,
        merchant_id: Address,
        business_name: String,
        settlement_currency: String,
    ) -> Result<(), Error> {
        merchant_id.require_auth();

        if env
            .storage()
            .persistent()
            .has(&DataKey::Merchant(merchant_id.clone()))
        {
            return Err(Error::MerchantAlreadyExists);
        }

        let merchant = Merchant {
            merchant_id: merchant_id.clone(),
            business_name,
            settlement_currency,
            kyc_tier: KycTier::Unverified,
            active: true,
            created_at: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Merchant(merchant_id.clone()), &merchant);

        // Add to merchant list for enumeration
        Self::add_to_merchant_list(&env, &merchant_id);

        Ok(())
    }

    /// Update merchant settings
    pub fn update_merchant(
        env: Env,
        merchant_id: Address,
        business_name: Option<String>,
        settlement_currency: Option<String>,
        active: Option<bool>,
    ) -> Result<(), Error> {
        merchant_id.require_auth();

        let mut merchant = Self::get_merchant_internal(&env, &merchant_id)?;

        if let Some(name) = business_name {
            merchant.business_name = name;
        }
        if let Some(currency) = settlement_currency {
            merchant.settlement_currency = currency;
        }
        if let Some(is_active) = active {
            merchant.active = is_active;
        }

        env.storage()
            .persistent()
            .set(&DataKey::Merchant(merchant_id), &merchant);

        Ok(())
    }

    /// Get merchant info
    pub fn get_merchant(env: Env, merchant_id: Address) -> Result<Merchant, Error> {
        Self::get_merchant_internal(&env, &merchant_id)
    }

    /// Verify merchant (admin only) — sets KycTier::Basic for backward compatibility.
    pub fn verify_merchant(env: Env, admin: Address, merchant_id: Address) -> Result<(), Error> {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;

        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        let mut merchant = Self::get_merchant_internal(&env, &merchant_id)?;
        merchant.kyc_tier = KycTier::Basic;

        env.storage()
            .persistent()
            .set(&DataKey::Merchant(merchant_id), &merchant);

        Ok(())
    }

    /// Set a specific KYC tier for a merchant (admin only).
    pub fn set_kyc_tier(
        env: Env,
        admin: Address,
        merchant_id: Address,
        tier: KycTier,
    ) -> Result<(), Error> {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;

        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        let mut merchant = Self::get_merchant_internal(&env, &merchant_id)?;
        merchant.kyc_tier = tier;

        env.storage()
            .persistent()
            .set(&DataKey::Merchant(merchant_id), &merchant);

        Ok(())
    }

    // Helper functions
    fn add_to_merchant_list(env: &Env, merchant_id: &Address) {
        let key = DataKey::MerchantList;
        let mut merchants: Vec<Address> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| vec![env]);

        // Only add if not already present
        let mut found = false;
        for m in merchants.iter() {
            if m == *merchant_id {
                found = true;
                break;
            }
        }

        if !found {
            merchants.push_back(merchant_id.clone());
            env.storage().persistent().set(&key, &merchants);
        }
    }

    /// Get all registered merchants with pagination support
    pub fn get_all_merchants(
        env: Env,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Merchant>, Error> {
        let merchant_ids: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::MerchantList)
            .unwrap_or_else(|| vec![&env]);

        if limit == 0 {
            return Ok(vec![&env]);
        }

        let mut result = vec![&env];
        let start = offset as usize;
        let end = core::cmp::min(merchant_ids.len(), start.saturating_add(limit as usize));

        let mut i = start;
        while i < end {
            if let Some(merchant_id) = merchant_ids.get(i) {
                if let Ok(merchant) = Self::get_merchant_internal(&env, &merchant_id) {
                    result.push_back(merchant);
                }
            }
            i += 1;
        }

        Ok(result)
    }

    /// Get all verified merchants (kyc_tier != Unverified)
    pub fn get_verified_merchants(env: Env) -> Result<Vec<Merchant>, Error> {
        let merchant_ids: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::MerchantList)
            .unwrap_or_else(|| vec![&env]);

        let mut result = vec![&env];
        for merchant_id in merchant_ids.iter() {
            if let Ok(merchant) = Self::get_merchant_internal(&env, &merchant_id) {
                if merchant.kyc_tier != KycTier::Unverified {
                    result.push_back(merchant);
                }
            }
        }

        Ok(result)
    }

    fn get_merchant_internal(env: &Env, merchant_id: &Address) -> Result<Merchant, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Merchant(merchant_id.clone()))
            .ok_or(Error::MerchantNotFound)
    }
}
