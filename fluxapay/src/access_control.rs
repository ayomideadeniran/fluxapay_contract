use soroban_sdk::{contracterror, contracttype, vec, Address, Env, Symbol, Vec};

// Role-based access control implementation
pub fn role_admin(env: &Env) -> Symbol {
    Symbol::new(env, "ADMIN")
}

pub fn role_oracle(env: &Env) -> Symbol {
    Symbol::new(env, "ORACLE")
}

#[allow(dead_code)]
pub fn role_merchant(env: &Env) -> Symbol {
    Symbol::new(env, "MERCHANT")
}

pub fn role_settlement_operator(env: &Env) -> Symbol {
    Symbol::new(env, "SETTLEMENT_OPERATOR")
}

#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessControlError {
    Unauthorized = 1,
    RoleAlreadyGranted = 2,
    RoleNotGranted = 3,
    CannotRenounceAdmin = 4,
    InvalidAdmin = 5,
}

#[contracttype]
pub enum AccessControlDataKey {
    Role(Symbol, Address),
    Admin,
    /// Stores the list of all addresses holding a given role.
    RoleMembers(Symbol),
}

pub struct AccessControl;

impl AccessControl {
    pub fn initialize(env: &Env, admin: Address) {
        env.storage()
            .persistent()
            .set(&AccessControlDataKey::Admin, &admin);
        Self::grant_role_internal(env, &role_admin(env), &admin);
    }

    pub fn grant_role(
        env: &Env,
        admin: Address,
        role: Symbol,
        account: Address,
    ) -> Result<(), AccessControlError> {
        admin.require_auth();
        if !Self::has_role(env, &role_admin(env), &admin) {
            return Err(AccessControlError::Unauthorized);
        }

        if Self::has_role(env, &role, &account) {
            return Err(AccessControlError::RoleAlreadyGranted);
        }

        Self::grant_role_internal(env, &role, &account);
        Ok(())
    }

    pub fn revoke_role(
        env: &Env,
        admin: Address,
        role: Symbol,
        account: Address,
    ) -> Result<(), AccessControlError> {
        admin.require_auth();
        if !Self::has_role(env, &role_admin(env), &admin) {
            return Err(AccessControlError::Unauthorized);
        }

        if !Self::has_role(env, &role, &account) {
            return Err(AccessControlError::RoleNotGranted);
        }

        Self::revoke_role_internal(env, &role, &account);
        Ok(())
    }

    pub fn has_role(env: &Env, role: &Symbol, account: &Address) -> bool {
        env.storage()
            .persistent()
            .get(&AccessControlDataKey::Role(role.clone(), account.clone()))
            .unwrap_or(false)
    }

    pub fn renounce_role(
        env: &Env,
        account: Address,
        role: Symbol,
    ) -> Result<(), AccessControlError> {
        if role == role_admin(env) {
            return Err(AccessControlError::CannotRenounceAdmin);
        }

        if !Self::has_role(env, &role, &account) {
            return Err(AccessControlError::RoleNotGranted);
        }

        Self::revoke_role_internal(env, &role, &account);
        Ok(())
    }

    pub fn transfer_admin(
        env: &Env,
        current_admin: Address,
        new_admin: Address,
    ) -> Result<(), AccessControlError> {
        current_admin.require_auth();
        if !Self::has_role(env, &role_admin(env), &current_admin) {
            return Err(AccessControlError::Unauthorized);
        }

        Self::revoke_role_internal(env, &role_admin(env), &current_admin);
        Self::grant_role_internal(env, &role_admin(env), &new_admin);

        env.storage()
            .persistent()
            .set(&AccessControlDataKey::Admin, &new_admin);

        Ok(())
    }

    pub fn get_admin(env: &Env) -> Option<Address> {
        env.storage().persistent().get(&AccessControlDataKey::Admin)
    }

    #[allow(dead_code)]
    pub fn require_role(
        env: &Env,
        role: &Symbol,
        account: &Address,
    ) -> Result<(), AccessControlError> {
        if !Self::has_role(env, role, account) {
            return Err(AccessControlError::Unauthorized);
        }
        Ok(())
    }

    /// Returns all addresses currently holding the given role.
    pub fn get_role_members(env: &Env, role: &Symbol) -> Vec<Address> {
        env.storage()
            .persistent()
            .get(&AccessControlDataKey::RoleMembers(role.clone()))
            .unwrap_or_else(|| vec![env])
    }

    fn grant_role_internal(env: &Env, role: &Symbol, account: &Address) {
        env.storage().persistent().set(
            &AccessControlDataKey::Role(role.clone(), account.clone()),
            &true,
        );

        // Maintain the role members index
        let key = AccessControlDataKey::RoleMembers(role.clone());
        let mut members: Vec<Address> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| vec![env]);

        // Only add if not already present (guard against double-add)
        let mut found = false;
        for m in members.iter() {
            if m == *account {
                found = true;
                break;
            }
        }
        if !found {
            members.push_back(account.clone());
            env.storage().persistent().set(&key, &members);
        }
    }

    fn revoke_role_internal(env: &Env, role: &Symbol, account: &Address) {
        env.storage()
            .persistent()
            .remove(&AccessControlDataKey::Role(role.clone(), account.clone()));

        // Remove from the role members index
        let key = AccessControlDataKey::RoleMembers(role.clone());
        let members: Vec<Address> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| vec![env]);

        let mut updated = vec![env];
        for m in members.iter() {
            if m != *account {
                updated.push_back(m);
            }
        }
        env.storage().persistent().set(&key, &updated);
    }
}
