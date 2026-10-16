//! Machine-readable resource admission comes from the authoritative core owner.
use crate::Result;
use metis_core::embedded_assets::{
    FILE_LIMIT, MANIFEST_LIMIT, MODULE_LIMIT, PAYLOAD_LIMIT, PORTABLE_PATH_LIMIT,
};
use serde::Serialize;
use std::io::Write;

#[derive(Serialize)]
struct Budgets {
    schema: u16,
    portable_path_bytes: usize,
    records: usize,
    manifest_bytes: u64,
    module_bytes: u64,
    payload_bytes: u64,
}

pub(crate) fn write(mut output: impl Write) -> Result<()> {
    serde_json::to_writer(
        &mut output,
        &Budgets {
            schema: 1,
            portable_path_bytes: PORTABLE_PATH_LIMIT,
            records: FILE_LIMIT,
            manifest_bytes: MANIFEST_LIMIT,
            module_bytes: MODULE_LIMIT,
            payload_bytes: PAYLOAD_LIMIT,
        },
    )?;
    output.write_all(b"\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budgets_report_contains_every_actual_core_bound() {
        crate::test_support::traced(|| {
            let mut bytes = {
                let _phase = tracing::info_span!("arrange").entered();
                Vec::new()
            };
            {
                let _phase = tracing::info_span!("act").entered();
                write(&mut bytes).expect("serialize actual budget owners");
            }
            let _phase = tracing::info_span!("assert").entered();
            assert_eq!(bytes.last(), Some(&b'\n'));
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).expect("bounded budget JSON");
            assert_eq!(
                value,
                serde_json::json!({ "schema":1, "portable_path_bytes":PORTABLE_PATH_LIMIT, "records":FILE_LIMIT, "manifest_bytes":MANIFEST_LIMIT, "module_bytes":MODULE_LIMIT, "payload_bytes":PAYLOAD_LIMIT })
            );
        });
    }
}
