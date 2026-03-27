use super::merchant_registry::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger, Address, Env, String};

#[test]
fn test_merchant_registration() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| li.timestamp = 1000);

    let contract_id = env.register(MerchantRegistry, ());
    let client = MerchantRegistryClient::new(&env, &contract_id);

    let merchant_id = Address::generate(&env);
    let business_name = String::from_str(&env, "Test Merchant");
    let settlement_currency = String::from_str(&env, "USDC");

    client.register_merchant(&merchant_id, &business_name, &settlement_currency);

    let merchant = client.get_merchant(&merchant_id);

    assert_eq!(merchant.merchant_id, merchant_id);
    assert_eq!(merchant.business_name, business_name);
    assert_eq!(merchant.settlement_currency, settlement_currency);
    // New: kyc_tier starts as Unverified
    assert_eq!(merchant.kyc_tier, KycTier::Unverified);
    assert!(merchant.active);
    assert!(merchant.created_at > 0);
}

#[test]
fn test_merchant_update() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MerchantRegistry, ());
    let client = MerchantRegistryClient::new(&env, &contract_id);

    let merchant_id = Address::generate(&env);
    let business_name = String::from_str(&env, "Initial name");
    let settlement_currency = String::from_str(&env, "USD");

    client.register_merchant(&merchant_id, &business_name, &settlement_currency);

    let new_name = String::from_str(&env, "New name");
    let new_currency = String::from_str(&env, "EUR");

    client.update_merchant(
        &merchant_id,
        &Some(new_name.clone()),
        &Some(new_currency.clone()),
        &Some(false),
    );

    let updated_merchant = client.get_merchant(&merchant_id);

    assert_eq!(updated_merchant.business_name, new_name);
    assert_eq!(updated_merchant.settlement_currency, new_currency);
    assert!(!updated_merchant.active);
}

#[test]
fn test_merchant_verification() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MerchantRegistry, ());
    let client = MerchantRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let merchant_id = Address::generate(&env);

    client.initialize(&admin);

    client.register_merchant(
        &merchant_id,
        &String::from_str(&env, "Merchant"),
        &String::from_str(&env, "USDC"),
    );

    // verify_merchant sets KycTier::Basic for backward compatibility
    client.verify_merchant(&admin, &merchant_id);

    let merchant = client.get_merchant(&merchant_id);
    assert_eq!(merchant.kyc_tier, KycTier::Basic);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #3)")]
fn test_unauthorized_verification() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MerchantRegistry, ());
    let client = MerchantRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let merchant_id = Address::generate(&env);

    client.initialize(&admin);

    client.register_merchant(
        &merchant_id,
        &String::from_str(&env, "Merchant"),
        &String::from_str(&env, "USDC"),
    );

    // Attacker tries to verify the merchant
    client.verify_merchant(&attacker, &merchant_id);
}

#[test]
fn test_set_kyc_tier() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MerchantRegistry, ());
    let client = MerchantRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let merchant_id = Address::generate(&env);

    client.initialize(&admin);
    client.register_merchant(
        &merchant_id,
        &String::from_str(&env, "BigCorp"),
        &String::from_str(&env, "USDC"),
    );

    // Promote through tiers
    client.set_kyc_tier(&admin, &merchant_id, &KycTier::Full);
    assert_eq!(client.get_merchant(&merchant_id).kyc_tier, KycTier::Full);

    client.set_kyc_tier(&admin, &merchant_id, &KycTier::Business);
    assert_eq!(
        client.get_merchant(&merchant_id).kyc_tier,
        KycTier::Business
    );
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #3)")]
fn test_set_kyc_tier_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(MerchantRegistry, ());
    let client = MerchantRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let merchant_id = Address::generate(&env);

    client.initialize(&admin);
    client.register_merchant(
        &merchant_id,
        &String::from_str(&env, "Merchant"),
        &String::from_str(&env, "USDC"),
    );

    // Non-admin tries to set KYC tier
    client.set_kyc_tier(&attacker, &merchant_id, &KycTier::Business);
}
