use crate::{
    manifest::{Application, EmbeddedPlatform},
    test_support::traced,
};

fn document() -> serde_json::Value {
    serde_json::to_value(super::fixture()).expect("valid actual native manifest value")
}

#[test]
fn optional_module_preserves_native_shape_and_exact_serialized_declaration() {
    traced(|| {
        let mut value = {
            let _phase = tracing::info_span!("arrange").entered();
            document()
        };
        value
            .as_object_mut()
            .expect("valid actual native manifest value")
            .remove("embedded_module");
        let (ordinary, embedded) = {
            let _phase = tracing::info_span!("act").entered();
            let ordinary: Application =
                serde_json::from_value(value.clone()).expect("valid actual native manifest value");
            value["embedded_module"] =
                serde_json::json!({"package":"metis-web", "platform":"windows"});
            let embedded: Application =
                serde_json::from_value(value).expect("valid actual native manifest value");
            (ordinary, embedded)
        };
        let _phase = tracing::info_span!("assert").entered();
        assert_eq!(
            serde_json::to_value(&ordinary)
                .expect("valid actual native manifest value")
                .get("embedded_module"),
            None
        );
        assert_eq!(ordinary.entry, embedded.entry);
        assert_eq!(ordinary.binaries.len(), embedded.binaries.len());
        assert_eq!(
            serde_json::to_value(&embedded).expect("valid actual native manifest value")["embedded_module"],
            serde_json::json!({"package":"metis-web", "platform":"windows"})
        );
        let module = embedded
            .embedded_module
            .expect("valid actual native manifest value");
        assert_eq!(module.package, "metis-web");
        assert!(matches!(module.platform, EmbeddedPlatform::Windows));
    });
}

#[test]
fn declaration_rejects_unknown_platform_fields_and_invalid_package_identity() {
    traced(|| {
        let base = {
            let _phase = tracing::info_span!("arrange").entered();
            document()
        };
        let errors = {
            let _phase = tracing::info_span!("act").entered();
            [
        (serde_json::json!({"package":"metis-web", "platform":"macos"}), "unknown variant"),
        (serde_json::json!({"package":"metis-web", "platform":"windows", "export":"other"}), "unknown field"),
    ].map(|(module, expected)| { let mut value = base.clone(); value["embedded_module"] = module; (serde_json::from_value::<Application>(value).expect_err("invalid embedded declaration").to_string(), expected) })
        };
        let _phase = tracing::info_span!("assert").entered();
        for (error, expected) in errors {
            assert!(error.contains(expected), "{error}");
        }
        let mut value = base;
        value["embedded_module"] =
            serde_json::json!({"package":"../metis-web", "platform":"windows"});
        assert_eq!(
            serde_json::from_value::<Application>(value)
                .expect("valid actual native manifest value")
                .validate()
                .expect_err("invalid embedded declaration")
                .to_string(),
            "identity/target must be a bounded ASCII identifier"
        );
    });
}
