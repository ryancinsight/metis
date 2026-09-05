//! Security and capability verification test suite.

use metis_core::capability::{CapabilityScope, CapabilityToken, VerifiedCapability};
use metis_core::error::ErrorCode;

const MASTER_KEY: [u8; 32] = [0x5A; 32];

#[test]
fn test_capability_issuance_and_verification() {
    let principal = [0x12; 16];
    let now = 1_700_000_000;
    let token = CapabilityToken::issue(
        1001,
        principal,
        CapabilityScope::SUBMIT_CALCULATION,
        now,
        3600,
        999,
        &MASTER_KEY,
    );

    // Valid verification
    token
        .verify(CapabilityScope::SUBMIT_CALCULATION, now + 100, &MASTER_KEY)
        .expect("valid capability");
    assert_eq!(token.principal_id, principal);

    // Proving with zero-sized witness type
    let proof = VerifiedCapability::<{ CapabilityScope::SUBMIT_CALCULATION.0 }>::prove(
        &token,
        now + 100,
        &MASTER_KEY,
    )
    .expect("Proof creation failed");
    assert_eq!(proof.token_id(), 1001);
}

#[test]
fn test_tampered_token_rejection() {
    let principal = [0x12; 16];
    let now = 1_700_000_000;
    let mut token = CapabilityToken::issue(
        1002,
        principal,
        CapabilityScope::SUBMIT_CALCULATION,
        now,
        3600,
        1,
        &MASTER_KEY,
    );

    // Tamper with scope (escalation attempt to SYSTEM_ADMIN)
    token.scope = CapabilityScope::SYSTEM_ADMIN;
    let err = token
        .verify(CapabilityScope::SYSTEM_ADMIN, now + 10, &MASTER_KEY)
        .expect_err("invalid input must be rejected");
    assert_eq!(err.code, ErrorCode::InvalidCapabilitySignature);

    // Tamper with principal ID
    token.principal_id[0] ^= 0xFF;
    let err2 = token
        .verify(CapabilityScope::SUBMIT_CALCULATION, now + 10, &MASTER_KEY)
        .expect_err("invalid input must be rejected");
    assert_eq!(err2.code, ErrorCode::InvalidCapabilitySignature);
}

#[test]
fn test_expired_token_rejection() {
    let principal = [0x12; 16];
    let now = 1_700_000_000;
    let token = CapabilityToken::issue(
        1003,
        principal,
        CapabilityScope::SUBMIT_CALCULATION,
        now,
        60, // expires in 60s
        2,
        &MASTER_KEY,
    );

    // 61 seconds later
    let err = token
        .verify(CapabilityScope::SUBMIT_CALCULATION, now + 61, &MASTER_KEY)
        .expect_err("invalid input must be rejected");
    assert_eq!(err.code, ErrorCode::CapabilityExpired);
}

#[test]
fn test_insufficient_scope_rejection() {
    let principal = [0x12; 16];
    let now = 1_700_000_000;
    let token = CapabilityToken::issue(
        1004,
        principal,
        CapabilityScope::UI_RENDER, // only UI_RENDER
        now,
        3600,
        3,
        &MASTER_KEY,
    );

    let err = token
        .verify(CapabilityScope::SUBMIT_CALCULATION, now + 10, &MASTER_KEY)
        .expect_err("invalid input must be rejected");
    assert_eq!(err.code, ErrorCode::InsufficientScope);
}

#[test]
fn capability_validity_is_half_open() {
    let token = CapabilityToken::issue(
        1,
        [1; 16],
        CapabilityScope::UI_RENDER,
        100,
        10,
        2,
        &MASTER_KEY,
    );
    for timestamp in [99, 110, u64::MAX] {
        assert_eq!(
            token
                .verify(CapabilityScope::UI_RENDER, timestamp, &MASTER_KEY)
                .expect_err("outside validity")
                .code,
            ErrorCode::CapabilityExpired
        );
    }
    for timestamp in [100, 109] {
        let witness = VerifiedCapability::<{ CapabilityScope::UI_RENDER.0 }>::prove(
            &token,
            timestamp,
            &MASTER_KEY,
        )
        .expect("inside validity");
        assert_eq!(witness.token_id(), 1);
    }
}
