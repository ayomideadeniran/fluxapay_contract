use soroban_sdk::{Bytes, Env, String};

/// Converts a `u64` counter to a Soroban `String` with the given prefix.
///
/// Examples: `format_id(env, "refund_", 1)` → `"refund_1"`
///           `format_id(env, "dispute_", 20)` → `"dispute_20"`
pub fn format_id(env: &Env, prefix: &str, n: u64) -> String {
    let mut result = Bytes::new(env);

    // Write prefix bytes
    for byte in prefix.as_bytes() {
        result.push_back(*byte);
    }

    // Build digits in reverse, then reverse them into result
    let mut temp = Bytes::new(env);
    let mut num = n;
    loop {
        temp.push_back((num % 10) as u8 + b'0');
        num /= 10;
        if num == 0 {
            break;
        }
    }
    let len = temp.len();
    for i in 0..len {
        result.push_back(temp.get(len - i - 1).unwrap());
    }

    // Copy into a fixed-size slice and convert to Soroban String
    let mut arr = [0u8; 64];
    let final_len = result.len().min(64);
    for i in 0..final_len {
        arr[i as usize] = result.get(i).unwrap();
    }
    String::from_bytes(env, &arr[..final_len as usize])
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    fn id_str<'a>(_env: &Env, s: &String, buf: &'a mut [u8; 64]) -> &'a str {
        let len = s.len() as usize;
        s.copy_into_slice(&mut buf[..len]);
        core::str::from_utf8(&buf[..len]).unwrap()
    }

    #[test]
    fn test_single_digit() {
        let env = Env::default();
        let mut buf = [0u8; 64];
        let id = format_id(&env, "refund_", 1);
        assert_eq!(id_str(&env, &id, &mut buf), "refund_1");
    }

    #[test]
    fn test_double_digit() {
        let env = Env::default();
        let mut buf = [0u8; 64];
        let id = format_id(&env, "refund_", 20);
        assert_eq!(id_str(&env, &id, &mut buf), "refund_20");
    }

    #[test]
    fn test_large_number() {
        let env = Env::default();
        let mut buf = [0u8; 64];
        let id = format_id(&env, "dispute_", 1_000_000);
        assert_eq!(id_str(&env, &id, &mut buf), "dispute_1000000");
    }

    #[test]
    fn test_zero() {
        let env = Env::default();
        let mut buf = [0u8; 64];
        let id = format_id(&env, "refund_", 0);
        assert_eq!(id_str(&env, &id, &mut buf), "refund_0");
    }

    #[test]
    fn test_u64_max() {
        let env = Env::default();
        let mut buf = [0u8; 64];
        let id = format_id(&env, "id_", u64::MAX);
        // u64::MAX = 18446744073709551615 (20 digits) + prefix "id_" (3) = 23 bytes, fits in 64
        assert_eq!(id_str(&env, &id, &mut buf), "id_18446744073709551615");
    }

    #[test]
    fn test_dispute_prefix() {
        let env = Env::default();
        let mut buf = [0u8; 64];
        let id = format_id(&env, "dispute_", 7);
        assert_eq!(id_str(&env, &id, &mut buf), "dispute_7");
    }

    #[test]
    fn test_uniqueness() {
        let env = Env::default();
        let id1 = format_id(&env, "refund_", 1);
        let id2 = format_id(&env, "refund_", 2);
        assert_ne!(id1, id2);
    }
}
