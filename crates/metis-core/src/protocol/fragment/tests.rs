//! Regression tests for the fragment wire protocol.

use super::*;
use crate::error::ErrorCode;

#[test]
fn action_round_trip_preserves_generation_and_input() {
    let action =
        FragmentAction::new(7, "status.describe", "metis-events", "session-患者").expect("action");
    assert_eq!(
        FragmentAction::decode(&action.encode().expect("encode")).expect("decode"),
        action
    );
}

#[test]
fn patch_set_round_trip_preserves_order_and_text_only_children() {
    let patches = vec![
        FragmentPatch::set_text("metis-events", "accepted").expect("text"),
        FragmentPatch::set_attribute("metis-status", "aria-busy", "false").expect("attribute"),
        FragmentPatch::replace_children("metis-events", "safe text").expect("children"),
    ];
    let patch_set = FragmentPatchSet::new(3, patches).expect("patch set");
    assert_eq!(
        FragmentPatchSet::decode(&patch_set.encode().expect("encode")).expect("decode"),
        patch_set
    );
}

#[test]
fn malformed_and_oversized_fields_are_rejected() {
    assert_eq!(
        FragmentAction::new(0, "status.describe", "metis-events", "input")
            .expect_err("zero generation")
            .code,
        ErrorCode::MalformedPayload
    );
    assert_eq!(
        FragmentAction::new(1, "Status.describe", "metis-events", "input")
            .expect_err("uppercase action")
            .code,
        ErrorCode::MalformedPayload
    );
    assert_eq!(
        FragmentPatch::set_attribute("metis-status", "onclick", "alert(1)")
            .expect("wire codec permits policy decision")
            .target(),
        "metis-status"
    );
    assert_eq!(
        FragmentPatchSet::new(
            1,
            (0..=MAX_FRAGMENT_PATCHES)
                .map(|_| FragmentPatch::set_text("metis-events", "x").expect("patch"))
                .collect(),
        )
        .expect_err("patch bound")
        .code,
        ErrorCode::PayloadTooLarge
    );
}

#[test]
fn patch_set_validates_public_variants_and_exact_body_bound() {
    let oversized = FragmentPatch::SetText {
        target: "metis-events".to_owned(),
        value: "x".repeat(MAX_FRAGMENT_VALUE_BYTES + 1),
    };
    assert_eq!(
        FragmentPatchSet::new(1, vec![oversized])
            .expect_err("oversized public variant")
            .code,
        ErrorCode::PayloadTooLarge
    );

    let invalid_attribute = FragmentPatch::SetAttribute {
        target: "metis-events".to_owned(),
        name: "data-!".to_owned(),
        value: "x".to_owned(),
    };
    assert_eq!(
        FragmentPatchSet::new(1, vec![invalid_attribute])
            .expect_err("invalid public variant")
            .code,
        ErrorCode::MalformedPayload
    );

    let patches = vec![
        FragmentPatch::SetText {
            target: "metis-events".to_owned(),
            value: "x".repeat(4096),
        },
        FragmentPatch::SetText {
            target: "metis-events".to_owned(),
            value: "x".repeat(4096),
        },
        FragmentPatch::SetText {
            target: "metis-events".to_owned(),
            value: "x".repeat(4096),
        },
        FragmentPatch::SetText {
            target: "metis-events".to_owned(),
            value: "x".repeat(4010),
        },
    ];
    assert_eq!(
        FragmentPatchSet::new(1, patches)
            .expect_err("exact body bound")
            .code,
        ErrorCode::PayloadTooLarge
    );
}

#[test]
fn action_decoder_rejects_trailing_bytes() {
    let action = FragmentAction::new(1, "status.describe", "metis-events", "x").expect("action");
    let mut encoded = action.encode().expect("encode");
    encoded.push(0);
    assert_eq!(
        FragmentAction::decode(&encoded)
            .expect_err("trailing bytes")
            .code,
        ErrorCode::MalformedPayload
    );
}

#[test]
fn patch_decoder_rejects_reserved_unknown_and_trailing_bytes() {
    let mut reserved = Vec::from(1_u64.to_be_bytes());
    reserved.extend_from_slice(&0_u16.to_be_bytes());
    reserved.extend_from_slice(&[1, 0]);
    assert_eq!(
        FragmentPatchSet::decode(&reserved)
            .expect_err("reserved bytes")
            .code,
        ErrorCode::MalformedPayload
    );

    let mut unknown = Vec::from(1_u64.to_be_bytes());
    unknown.extend_from_slice(&1_u16.to_be_bytes());
    unknown.extend_from_slice(&[0, 0, 9]);
    assert_eq!(
        FragmentPatchSet::decode(&unknown)
            .expect_err("unknown patch kind")
            .code,
        ErrorCode::MalformedPayload
    );

    let patch_set = FragmentPatchSet::new(
        1,
        vec![FragmentPatch::set_text("metis-events", "text").expect("patch")],
    )
    .expect("patch set");
    let mut trailing = patch_set.encode().expect("encode");
    trailing.push(0);
    assert_eq!(
        FragmentPatchSet::decode(&trailing)
            .expect_err("trailing bytes")
            .code,
        ErrorCode::MalformedPayload
    );
}
