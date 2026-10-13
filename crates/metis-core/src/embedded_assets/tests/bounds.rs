use super::*;

#[test]
fn encoded_budget_admits_exact_limit_and_rejects_one_byte_more() {
    traced(|| {
        let mut value = {
            let _phase = tracing::info_span!("arrange").entered();
            spec()
        };
        let baseline = EmbeddedModuleInventory::new(value.clone())
            .expect("analytically valid contract value")
            .encode()
            .expect("analytically valid contract value")
            .len();
        let extra =
            usize::try_from(MANIFEST_LIMIT).expect("analytically valid contract value") - baseline;
        value.configuration.compiler_args[0].push_str(&"x".repeat(extra));
        let exact = {
            let _phase = tracing::info_span!("act").entered();
            EmbeddedModuleInventory::new(value.clone())
                .expect("analytically valid contract value")
                .encode()
                .expect("analytically valid contract value")
        };
        let _phase = tracing::info_span!("assert").entered();
        assert_eq!(
            u64::try_from(exact.len()).expect("analytically valid contract value"),
            MANIFEST_LIMIT
        );
        assert_eq!(
            EmbeddedModuleInventory::decode(&exact)
                .expect("analytically valid contract value")
                .spec(),
            &value
        );
        value.configuration.compiler_args[0].push('x');
        assert_eq!(
            EmbeddedModuleInventory::new(value)
                .expect_err("adversarial value must be rejected")
                .to_string(),
            "encoded inventory exceeds 1 MiB"
        );
    });
}

#[test]
fn aggregate_record_budget_and_count_overflow_are_rejected_before_allocation() {
    traced(|| {
        let mut value = {
            let _phase = tracing::info_span!("arrange").entered();
            spec()
        };
        value.inputs = (0..FILE_LIMIT - 3)
            .map(|index| InputRecord {
                owner_id: "workspace".into(),
                path: format!("src/{index:04}.rs"),
                identity: identity(&index.to_be_bytes()),
            })
            .collect();
        let admitted = {
            let _phase = tracing::info_span!("act").entered();
            EmbeddedModuleInventory::new(value.clone()).expect("analytically valid contract value")
        };
        let _phase = tracing::info_span!("assert").entered();
        assert_eq!(
            admitted.spec().inputs.len()
                + admitted.spec().providers.len()
                + admitted.spec().assets.len(),
            FILE_LIMIT
        );
        value.inputs.push(InputRecord {
            owner_id: "workspace".into(),
            path: "src/4093.rs".into(),
            identity: identity(b"record beyond exact aggregate bound"),
        });
        assert_eq!(
            EmbeddedModuleInventory::new(value)
                .expect_err("adversarial value must be rejected")
                .to_string(),
            "inventory requires bounded nonempty provider/input records"
        );
        let mut wire = b"METISAS1\0\x01".to_vec();
        wire.extend_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(
            EmbeddedModuleInventory::decode(&wire)
                .expect_err("adversarial value must be rejected")
                .to_string(),
            "truncated inventory field"
        );
    });
}

#[test]
fn canonical_wire_property_preserves_lengths_hashes_and_environment_presence() {
    traced(|| {
        let dimensions = {
            let _phase = tracing::info_span!("arrange").entered();
            [1_usize, 7, 31, 255, 4096, PORTABLE_PATH_LIMIT]
        };
        let outputs = {
            let _phase = tracing::info_span!("act").entered();
            dimensions.map(|length| {
                let mut value = spec();
                let bytes = (0..length)
                    .map(|index| {
                        u8::try_from(index % 251).expect("analytically valid contract value")
                    })
                    .collect::<Vec<_>>();
                value.inputs[0].identity = identity(&bytes);
                value.configuration.environment = vec![
                    ("EMPTY".into(), Some(String::new())),
                    ("UNSET".into(), None),
                ];
                let inventory = EmbeddedModuleInventory::new(value.clone())
                    .expect("analytically valid contract value");
                let wire = inventory
                    .encode()
                    .expect("analytically valid contract value");
                (value, wire, bytes)
            })
        };
        let _phase = tracing::info_span!("assert").entered();
        for (expected, wire, bytes) in outputs {
            let decoded =
                EmbeddedModuleInventory::decode(&wire).expect("analytically valid contract value");
            assert_eq!(decoded.spec(), &expected);
            assert_eq!(decoded.spec().inputs[0].identity, identity(&bytes));
            assert_eq!(
                decoded.spec().configuration.environment[0].1,
                Some(String::new())
            );
            assert_eq!(decoded.spec().configuration.environment[1].1, None);
            assert_eq!(
                decoded.encode().expect("analytically valid contract value"),
                wire
            );
        }
    });
}
