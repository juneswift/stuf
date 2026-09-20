#![cfg(feature = "no-heap")]

#[cfg(feature = "alloc")]
mod common;

#[cfg(feature = "alloc")]
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use stuf_env::clock::FixedClock;
use stuf_env::transport::Transport;
#[cfg(feature = "alloc")]
use stuf_tuf::error::Error;
use stuf_tuf::verify::no_heap::TrustAnchor;

#[cfg(feature = "alloc")]
use common::*;

const NOW: u64 = 1_700_000_000;

#[derive(Clone, Copy, Debug)]
struct NoTransport;

#[derive(Clone, Copy, Debug)]
struct NoTransportError;

impl Transport for NoTransport {
    type Buffer = &'static [u8];
    type Error = NoTransportError;

    fn fetch(&self, _id: &str) -> Result<Self::Buffer, Self::Error> {
        Err(NoTransportError)
    }
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/no_heap")
}

fn read(path: &str) -> Vec<u8> {
    fs::read(fixture_dir().join(path)).unwrap_or_else(|e| {
        panic!("failed to read fixture {path}: {e}");
    })
}

#[test]
fn no_heap_root_verifies_against_publisher_output() {
    let root = read("root.json");

    let _anchor =
        TrustAnchor::new(&root, NoTransport, FixedClock(NOW)).expect("no-heap root should verify");
}

#[test]
fn no_heap_full_chain_verifies_against_publisher_output() {
    let root = read("root.json");
    let timestamp = read("timestamp.json");
    let snapshot = read("snapshot.json");
    let targets = read("targets.json");
    let firmware = read("toaster-firmware-1.1.0.bin");

    let anchor =
        TrustAnchor::new(&root, NoTransport, FixedClock(NOW)).expect("no-heap root should verify");

    let timestamp = anchor
        .verify_timestamp_bytes(&timestamp)
        .expect("no-heap timestamp should verify");

    let snapshot = timestamp
        .verify_snapshot_bytes(&snapshot)
        .expect("no-heap snapshot should verify");

    let targets = snapshot
        .verify_targets_bytes(&targets)
        .expect("no-heap targets should verify");

    let verified = targets
        .verify_target_bytes("toaster-firmware-1.1.0.bin", &firmware)
        .expect("no-heap firmware should verify");

    assert_eq!(verified.into_inner().length, firmware.len() as u64);
}

#[cfg(feature = "alloc")]
fn make_no_heap_chain_with_target_hash(
    sha256: Option<String>,
) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let rk = TestKey::generate();
    let tk = TestKey::generate();
    let sk = TestKey::generate();
    let tsk = TestKey::generate();

    let root = make_root(&rk, &tk, &sk, &tsk, FAR_FUTURE, 1);
    let root_bytes = sign_root(&root, &rk);

    let ts = make_timestamp(1, FAR_FUTURE, 1);
    let snap = make_snapshot(1, FAR_FUTURE, 1);
    let targets = make_targets_with_hash(FIRMWARE, sha256, None, FAR_FUTURE, 1);

    (
        root_bytes,
        sign_timestamp(&ts, &tsk),
        sign_snapshot(&snap, &sk),
        sign_targets(&targets, &tk),
    )
}

#[cfg(feature = "alloc")]
#[test]
fn no_heap_truncated_target_sha256_reports_invalid_hash_length() {
    let short_hash = "abcd1234abcd1234abcd1234abcd1234".to_string();
    let (root, timestamp, snapshot, targets) =
        make_no_heap_chain_with_target_hash(Some(short_hash));

    let checked = TrustAnchor::new(&root, NoTransport, FixedClock(common::NOW))
        .expect("root should verify")
        .verify_timestamp_bytes(&timestamp)
        .expect("timestamp should verify")
        .verify_snapshot_bytes(&snapshot)
        .expect("snapshot should verify")
        .verify_targets_bytes(&targets)
        .expect("targets should verify");

    assert!(matches!(
        checked.verify_target_bytes("firmware.bin", FIRMWARE),
        Err(Error::InvalidHashLength {
            expected: 64,
            actual: 32
        })
    ));
}

#[cfg(feature = "alloc")]
#[test]
fn no_heap_non_hex_target_sha256_reports_invalid_hash_encoding() {
    let bad_hex = "z".repeat(64);
    let (root, timestamp, snapshot, targets) = make_no_heap_chain_with_target_hash(Some(bad_hex));

    let checked = TrustAnchor::new(&root, NoTransport, FixedClock(common::NOW))
        .expect("root should verify")
        .verify_timestamp_bytes(&timestamp)
        .expect("timestamp should verify")
        .verify_snapshot_bytes(&snapshot)
        .expect("snapshot should verify")
        .verify_targets_bytes(&targets)
        .expect("targets should verify");

    let err = checked
        .verify_target_bytes("firmware.bin", FIRMWARE)
        .unwrap_err();
    eprintln!("non-hex target sha256 err: {err:?}");
    assert!(matches!(err, Error::InvalidHashEncoding));
}

#[cfg(feature = "alloc")]
#[test]
fn no_heap_wrong_target_sha256_reports_target_hash_mismatch() {
    let wrong_hash = "a".repeat(64);
    let (root, timestamp, snapshot, targets) =
        make_no_heap_chain_with_target_hash(Some(wrong_hash));

    let checked = TrustAnchor::new(&root, NoTransport, FixedClock(common::NOW))
        .expect("root should verify")
        .verify_timestamp_bytes(&timestamp)
        .expect("timestamp should verify")
        .verify_snapshot_bytes(&snapshot)
        .expect("snapshot should verify")
        .verify_targets_bytes(&targets)
        .expect("targets should verify");

    assert!(matches!(
        checked.verify_target_bytes("firmware.bin", FIRMWARE),
        Err(Error::TargetHashMismatch)
    ));
}

#[cfg(feature = "alloc")]
#[test]
fn no_heap_bad_snapshot_metadata_hash_length_reports_invalid_hash_length() {
    let rk = TestKey::generate();
    let tk = TestKey::generate();
    let sk = TestKey::generate();
    let tsk = TestKey::generate();

    let root = make_root(&rk, &tk, &sk, &tsk, FAR_FUTURE, 1);
    let root_bytes = sign_root(&root, &rk);

    let snap = make_snapshot(1, FAR_FUTURE, 1);
    let snap_bytes = sign_snapshot(&snap, &sk);

    let mut bad_hash = BTreeMap::new();
    bad_hash.insert("sha256".to_string(), "abcd1234".to_string());

    let ts = make_timestamp_with_hash(1, FAR_FUTURE, 1, Some(bad_hash), None);
    let ts_bytes = sign_timestamp(&ts, &tsk);

    let anchor = TrustAnchor::new(&root_bytes, NoTransport, FixedClock(common::NOW)).unwrap();

    assert!(matches!(
        anchor
            .verify_timestamp_bytes(&ts_bytes)
            .unwrap()
            .verify_snapshot_bytes(&snap_bytes),
        Err(Error::InvalidHashLength { expected: 64, .. })
    ));
}

#[cfg(feature = "alloc")]
#[test]
fn no_heap_wrong_snapshot_metadata_hash_reports_metadata_hash_mismatch() {
    let rk = TestKey::generate();
    let tk = TestKey::generate();
    let sk = TestKey::generate();
    let tsk = TestKey::generate();

    let root = make_root(&rk, &tk, &sk, &tsk, FAR_FUTURE, 1);
    let root_bytes = sign_root(&root, &rk);

    let snap = make_snapshot(1, FAR_FUTURE, 1);
    let snap_bytes = sign_snapshot(&snap, &sk);

    let mut bad_hash = BTreeMap::new();
    bad_hash.insert("sha256".to_string(), "a".repeat(64));

    let ts = make_timestamp_with_hash(1, FAR_FUTURE, 1, Some(bad_hash), None);
    let ts_bytes = sign_timestamp(&ts, &tsk);

    let anchor = TrustAnchor::new(&root_bytes, NoTransport, FixedClock(common::NOW)).unwrap();

    assert!(matches!(
        anchor
            .verify_timestamp_bytes(&ts_bytes)
            .unwrap()
            .verify_snapshot_bytes(&snap_bytes),
        Err(Error::MetadataHashMismatch)
    ));
}

#[cfg(feature = "alloc")]
#[test]
fn no_heap_bad_metadata_length_reports_metadata_length_mismatch() {
    let rk = TestKey::generate();
    let tk = TestKey::generate();
    let sk = TestKey::generate();
    let tsk = TestKey::generate();

    let root = make_root(&rk, &tk, &sk, &tsk, FAR_FUTURE, 1);
    let root_bytes = sign_root(&root, &rk);

    let snap = make_snapshot(1, FAR_FUTURE, 1);
    let snap_bytes = sign_snapshot(&snap, &sk);

    let ts = make_timestamp_with_hash(1, FAR_FUTURE, 1, None, Some(1));
    let ts_bytes = sign_timestamp(&ts, &tsk);

    let anchor = TrustAnchor::new(&root_bytes, NoTransport, FixedClock(common::NOW)).unwrap();

    assert!(matches!(
        anchor
            .verify_timestamp_bytes(&ts_bytes)
            .unwrap()
            .verify_snapshot_bytes(&snap_bytes),
        Err(Error::MetadataLengthMismatch { .. })
    ));
}
