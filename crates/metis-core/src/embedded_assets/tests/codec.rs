use super::*;

#[test]
fn canonical_encoding_roundtrips_every_computed_field() {
    traced(|| {
        let expected = {
            let _phase = tracing::info_span!("arrange").entered();
            spec()
        };
        let (inventory, bytes, decoded) = {
            let _phase = tracing::info_span!("act").entered();
            let inventory = EmbeddedModuleInventory::new(expected.clone())
                .expect("analytically valid contract value");
            let bytes = inventory
                .encode()
                .expect("analytically valid contract value");
            let decoded =
                EmbeddedModuleInventory::decode(&bytes).expect("analytically valid contract value");
            (inventory, bytes, decoded)
        };
        let _phase = tracing::info_span!("assert").entered();
        assert_eq!(decoded.spec(), &expected);
        assert_eq!(&bytes[..10], b"METISAS1\0\x01");
        assert_eq!(
            decoded.encode().expect("analytically valid contract value"),
            bytes
        );
        assert_eq!(
            inventory
                .digest()
                .expect("analytically valid contract value"),
            Digest::hash(&bytes)
        );
        assert_eq!(decoded.spec().assets[1].identity.bytes, 8);
    });
}

#[test]
fn digest_matches_published_sha256_and_canonical_hex() {
    traced(|| {
        let text = {
            let _phase = tracing::info_span!("arrange").entered();
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        };
        let digest = {
            let _phase = tracing::info_span!("act").entered();
            Digest::hash(b"abc")
        };
        let _phase = tracing::info_span!("assert").entered();
        assert_eq!(digest.to_string(), text);
        assert_eq!(
            text.parse::<Digest>()
                .expect("analytically valid contract value")
                .into_bytes(),
            digest.into_bytes()
        );
        for invalid in [
            text.to_uppercase(),
            format!(" {text}"),
            text[..63].to_owned(),
            "g".repeat(64),
        ] {
            assert_eq!(
                invalid
                    .parse::<Digest>()
                    .expect_err("adversarial value must be rejected")
                    .kind(),
                std::io::ErrorKind::InvalidData
            );
        }
    });
}

#[test]
fn rejects_each_truncation_unknown_schema_and_trailing_bytes() {
    traced(|| {
        let bytes = {
            let _phase = tracing::info_span!("arrange").entered();
            EmbeddedModuleInventory::new(spec())
                .expect("analytically valid contract value")
                .encode()
                .expect("analytically valid contract value")
        };
        let errors = {
            let _phase = tracing::info_span!("act").entered();
            (0..bytes.len())
                .map(|end| {
                    EmbeddedModuleInventory::decode(&bytes[..end])
                        .expect_err("adversarial value must be rejected")
                        .kind()
                })
                .collect::<Vec<_>>()
        };
        let _phase = tracing::info_span!("assert").entered();
        assert_eq!(errors.len(), bytes.len());
        assert!(
            errors
                .iter()
                .all(|kind| *kind == std::io::ErrorKind::InvalidData)
        );
        let mut unknown = bytes.clone();
        unknown[9] = 2;
        assert_eq!(
            EmbeddedModuleInventory::decode(&unknown)
                .expect_err("adversarial value must be rejected")
                .to_string(),
            "unsupported inventory magic/schema"
        );
        let mut trailing = bytes;
        trailing.push(1);
        assert_eq!(
            EmbeddedModuleInventory::decode(&trailing)
                .expect_err("adversarial value must be rejected")
                .to_string(),
            "trailing inventory bytes"
        );
    });
}

#[test]
fn rejects_collisions_sorting_and_analytical_resource_boundaries() {
    traced(|| {
        let baseline = {
            let _phase = tracing::info_span!("arrange").entered();
            spec()
        };
        let mut cases = Vec::new();
        {
            let _phase = tracing::info_span!("act").entered();
            let mut value = baseline.clone();
            value.inputs.push(InputRecord {
                owner_id: "workspace".into(),
                path: "cargo.lock".into(),
                identity: identity(b"version = 4\n"),
            });
            cases.push(value);
            let mut value = baseline.clone();
            value.assets.swap(0, 1);
            cases.push(value);
            let mut value = baseline.clone();
            value.assets[0].identity.bytes = MODULE_LIMIT + 1;
            cases.push(value);
            let mut value = baseline.clone();
            value.inputs[0].identity.bytes = PAYLOAD_LIMIT;
            cases.push(value);
            let mut value = baseline.clone();
            value.inputs[0].path = "../Cargo.lock".into();
            cases.push(value);
            let mut value = baseline.clone();
            value.module.package_id = "path+file:///host/metis-web".into();
            cases.push(value);
        }
        let _phase = tracing::info_span!("assert").entered();
        for (value, expected) in cases.into_iter().zip([
            "collision",
            "sorted",
            "64 MiB",
            "1 GiB",
            "ambiguous",
            "absolute path",
        ]) {
            assert!(
                EmbeddedModuleInventory::new(value)
                    .expect_err("adversarial value must be rejected")
                    .to_string()
                    .contains(expected)
            );
        }
        let mut boundary = baseline;
        boundary.assets[0].identity.bytes = MODULE_LIMIT;
        assert_eq!(
            EmbeddedModuleInventory::new(boundary)
                .expect("analytically valid contract value")
                .spec()
                .assets[0]
                .identity
                .bytes,
            MODULE_LIMIT
        );
    });
}
