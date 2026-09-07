#[cfg(test)]
use std::str::FromStr;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use cln_rpc::primitives::PublicKey;
use parking_lot::Mutex;

use crate::structs::AbuseEntry;

pub const ABUSE_THRESHOLD: u32 = 3;
// Gossip can be empty/lacking right after node startup, which would make every
// peer look like it has no announcement/channels. Keep the throttle window
// short so a false positive can only lock a peer out briefly, while a heavy
// spammer that keeps retrying is still blocked.
pub const ABUSE_WINDOW_SECS: u64 = 2 * 60 * 60;
// Notifications (emails) about an abuser are sent at most once per interval,
// no matter how often they retry. Entries are kept this long so the cooldown
// survives across throttle windows.
pub const ABUSE_NOTIFY_INTERVAL_SECS: u64 = 24 * 60 * 60;
pub const MAX_ABUSE_ENTRIES: usize = 1_000;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn check_abuse(cache: &Arc<Mutex<HashMap<PublicKey, AbuseEntry>>>, pubkey: PublicKey) -> bool {
    let cache = cache.lock();
    match cache.get(&pubkey) {
        Some(entry) => {
            if now() - entry.last_try > ABUSE_WINDOW_SECS {
                false
            } else {
                entry.tries >= ABUSE_THRESHOLD
            }
        }
        None => false,
    }
}

pub fn record_abuse(cache: &Arc<Mutex<HashMap<PublicKey, AbuseEntry>>>, pubkey: PublicKey) {
    let mut cache = cache.lock();
    let unix_now_s = now();
    match cache.get_mut(&pubkey) {
        Some(entry) => {
            if unix_now_s - entry.last_try > ABUSE_WINDOW_SECS {
                // New throttle window: start counting again, keep the
                // notification cooldown from the previous window.
                entry.tries = 1;
                entry.last_try = unix_now_s;
            } else {
                entry.tries = entry.tries.saturating_add(1);
                entry.last_try = unix_now_s;
            }
        }
        None => {
            cache.insert(
                pubkey,
                AbuseEntry {
                    tries: 1,
                    last_try: unix_now_s,
                    last_notified: 0,
                },
            );
        }
    }
    evict_oldest(&mut cache);
}

fn evict_oldest(cache: &mut HashMap<PublicKey, AbuseEntry>) {
    if cache.len() > MAX_ABUSE_ENTRIES {
        if let Some(oldest_key) = cache
            .iter()
            .min_by_key(|(_, v)| v.last_try)
            .map(|(k, _)| *k)
        {
            cache.remove(&oldest_key);
        }
    }
}

pub fn clear_abuse(cache: &Arc<Mutex<HashMap<PublicKey, AbuseEntry>>>, pubkey: PublicKey) {
    cache.lock().remove(&pubkey);
}

/// Whether a notification (email) may be sent about this peer. Every peer,
/// abuser or not, is notified at most once per ABUSE_NOTIFY_INTERVAL_SECS.
/// Unknown peers are added to the cache so the cooldown also applies to
/// rejections that never went through the gossip-based abuse detection, for
/// example when no custom rule is configured.
pub fn should_notify(
    cache: &Arc<Mutex<HashMap<PublicKey, AbuseEntry>>>,
    pubkey: PublicKey,
) -> bool {
    let mut cache = cache.lock();
    let unix_now_s = now();
    match cache.get_mut(&pubkey) {
        Some(entry) => {
            if unix_now_s - entry.last_notified >= ABUSE_NOTIFY_INTERVAL_SECS {
                entry.last_notified = unix_now_s;
                true
            } else {
                false
            }
        }
        None => {
            cache.insert(
                pubkey,
                AbuseEntry {
                    tries: 0,
                    last_try: unix_now_s,
                    last_notified: unix_now_s,
                },
            );
            evict_oldest(&mut cache);
            true
        }
    }
}

pub fn evict_abuse(cache: &Arc<Mutex<HashMap<PublicKey, AbuseEntry>>>) {
    let unix_now_s = now();
    let mut cache = cache.lock();
    cache.retain(|_, v| unix_now_s - v.last_try.max(v.last_notified) <= ABUSE_NOTIFY_INTERVAL_SECS);
    if cache.len() > MAX_ABUSE_ENTRIES {
        let overflow = cache.len() - MAX_ABUSE_ENTRIES;
        for _ in 0..overflow {
            if let Some(oldest_key) = cache
                .iter()
                .min_by_key(|(_, v)| v.last_try)
                .map(|(k, _)| *k)
            {
                cache.remove(&oldest_key);
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PUBKEY: &str = "0380ef0209ff1b46c38a37cd40f613d1dae3eba481a909459d6c1434a0e56e5d8c";

    fn test_cache() -> Arc<Mutex<HashMap<PublicKey, AbuseEntry>>> {
        Arc::new(Mutex::new(HashMap::new()))
    }

    fn test_pubkey() -> PublicKey {
        PublicKey::from_str(TEST_PUBKEY).unwrap()
    }

    #[test]
    fn test_throttle_after_three() {
        let cache = test_cache();
        let pk = test_pubkey();
        assert!(!check_abuse(&cache, pk));
        record_abuse(&cache, pk);
        record_abuse(&cache, pk);
        assert!(!check_abuse(&cache, pk));
        record_abuse(&cache, pk);
        assert!(check_abuse(&cache, pk));
    }

    #[test]
    fn test_throttle_window_expiry_resets_tries() {
        let cache = test_cache();
        let pk = test_pubkey();
        for _ in 0..3 {
            record_abuse(&cache, pk);
        }
        assert!(check_abuse(&cache, pk));

        // Simulate the throttle window expiring.
        {
            let mut c = cache.lock();
            c.get_mut(&pk).unwrap().last_try = now() - ABUSE_WINDOW_SECS - 1;
        }
        assert!(!check_abuse(&cache, pk));

        // The next record starts a fresh window with a single try.
        record_abuse(&cache, pk);
        let c = cache.lock();
        assert_eq!(c.get(&pk).unwrap().tries, 1);
    }

    #[test]
    fn test_notify_cooldown_24h() {
        let cache = test_cache();
        let pk = test_pubkey();
        // A peer that is not a known abuser is still throttled: the first
        // notification is allowed, further ones within the interval are not.
        assert!(should_notify(&cache, pk));
        assert!(!should_notify(&cache, pk));

        // Simulate 24h passing.
        {
            let mut c = cache.lock();
            c.get_mut(&pk).unwrap().last_notified = now() - ABUSE_NOTIFY_INTERVAL_SECS - 1;
        }
        assert!(should_notify(&cache, pk));
    }

    #[test]
    fn test_notify_cooldown_survives_window_reset() {
        let cache = test_cache();
        let pk = test_pubkey();
        record_abuse(&cache, pk);
        assert!(should_notify(&cache, pk));
        assert!(!should_notify(&cache, pk));

        // The throttle window expires and a new burst starts, but the
        // 24h notification cooldown is kept.
        {
            let mut c = cache.lock();
            c.get_mut(&pk).unwrap().last_try = now() - ABUSE_WINDOW_SECS - 1;
        }
        record_abuse(&cache, pk);
        assert!(!should_notify(&cache, pk));
    }

    #[test]
    fn test_notify_only_entry_does_not_abuse() {
        let cache = test_cache();
        let pk = test_pubkey();
        // Being notified once must not count as an abuse attempt.
        assert!(should_notify(&cache, pk));
        assert!(!check_abuse(&cache, pk));
        // Abuse tracking can still build up afterwards.
        for _ in 0..3 {
            record_abuse(&cache, pk);
        }
        assert!(check_abuse(&cache, pk));
    }

    #[test]
    fn test_clear_abuse_resets_notify() {
        let cache = test_cache();
        let pk = test_pubkey();
        record_abuse(&cache, pk);
        assert!(should_notify(&cache, pk));
        assert!(!should_notify(&cache, pk));
        clear_abuse(&cache, pk);
        assert!(should_notify(&cache, pk));
    }
}
