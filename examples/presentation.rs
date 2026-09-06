//! Captures real form transitions through the backend and production framebuffer.
use metis_backend::{BackendService, clinical::SafetyEnvelope};
use metis_core::{ErrorCode, MetisError};
use metis_frontend::{FormState, FrontendApp};
use metis_ipc::{server::IpcServer, transport::MemoryTransport};
use metis_platform::{FONT_HEIGHT, FONT_WIDTH, Framebuffer};
use metis_ui_lang::{DisplayCommand, compute_layout};
use moirai_core::TaskSpawner;
use moirai_executor::ExecutorBuilder;
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all("output")?;
    let (mut app, service) = session_trace(4, |app| {
        assert_idle(app);
        assert_label(app, "status-badge", "SYSTEM READY");
        capture(app, "form")?;
        app.init(1234, [1; 16])?;
        app.set_inputs("demo", 60.0, 2.0, 0.2)?;
        app.submit_calculation()?;
        assert_result(app, 60.0, 0.36, 0.72, 2);
        capture(app, "form-success")?;

        app.set_inputs("demo", 80.0, 2.0, 0.2)?;
        assert_idle(app);
        assert_label(app, "label-weight", "Weight: 80.00 kg");
        capture(app, "form-edited")?;

        app.set_inputs("demo", 0.0, 2.0, 0.2)?;
        app.submit_calculation()?;
        let FormState::Rejected(error) = app.state() else {
            return Err("Expected backend rejection of zero weight".into());
        };
        assert_eq!(error.error_code, ErrorCode::InvalidPatientWeight as u16);
        assert_no_result(app, "Backend rejected request [0x3001]");
        capture(app, "form-rejected")?;

        app.set_inputs("demo", 60.0, 2.0, 0.2)?;
        app.submit_calculation()?;
        assert_result(app, 60.0, 0.36, 0.72, 4);
        capture(app, "form-corrected")
    })?;
    assert_eq!(
        service
            .ledger()
            .records()
            .iter()
            .map(|record| (record.sequence_id, record.outcome))
            .collect::<Vec<_>>(),
        [
            (1, None),
            (2, None),
            (3, Some(ErrorCode::InvalidPatientWeight)),
            (4, None)
        ]
    );
    // session_trace joins the finite worker: the peer has actually closed.
    let error = app
        .submit_calculation()
        .expect_err("closed backend endpoint");
    assert_eq!(error.code, ErrorCode::ConnectionClosed);
    assert_eq!(app.state(), &FormState::Disconnected(error));
    assert_no_result(&app, "Connection failed [0x4002] - reconnect");
    assert_label(&app, "status-badge", "SESSION CLOSED");
    capture(&app, "form-disconnected")?;

    let (_, recovered) = session_trace(2, |app| {
        app.init(1234, [1; 16])?;
        app.set_inputs("demo", 80.0, 2.0, 0.2)?;
        app.submit_calculation()?;
        assert_result(app, 80.0, 0.48, 0.96, 2);
        capture(app, "form-recovered")
    })?;
    assert_eq!(recovered.ledger().records().len(), 2);
    Ok(())
}

fn session_trace(
    request_count: usize,
    trace: impl FnOnce(&mut FrontendApp<MemoryTransport>) -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(FrontendApp<MemoryTransport>, BackendService), Box<dyn std::error::Error>> {
    let (front, back) = MemoryTransport::pair();
    let mut executor = ExecutorBuilder::new()
        .worker_threads(1)
        .async_threads(1)
        .build()?;
    let server = executor.spawn_blocking(move || -> Result<BackendService, MetisError> {
        // A public fixture key is confined to this synthetic demonstration.
        let mut service = BackendService::new([0x77; 32], SafetyEnvelope::default());
        let mut server = IpcServer::new(back);
        for _ in 0..request_count {
            if !server.step(&mut service)? {
                return Err(MetisError::transport(
                    ErrorCode::ConnectionClosed,
                    "Demonstration ended before its expected exchanges",
                ));
            }
        }
        service.ledger().verify_chain()?;
        Ok(service)
    })?;
    let mut app = FrontendApp::new(front, 800, 600)?;
    let outcome = trace(&mut app);
    let completion = server.join();
    executor.shutdown()?;
    outcome?;
    let service = completion.ok_or("Backend task lost its result handle")???;
    Ok((app, service))
}

fn assert_label(app: &FrontendApp<MemoryTransport>, id: &str, expected: &str) {
    let element = app
        .document()
        .find_element_by_id(id)
        .expect("authored form label");
    assert_eq!(element.text_content(), expected, "label {id}");
}

fn assert_idle(app: &FrontendApp<MemoryTransport>) {
    assert_eq!(app.state(), &FormState::Idle);
    assert_label(app, "output-rate", "Rate: Awaiting Backend Calculation...");
    assert_label(app, "output-status", "Safety Status: Idle");
    assert_label(app, "output-signature", "Backend MAC: None");
}

fn assert_no_result(app: &FrontendApp<MemoryTransport>, status: &str) {
    assert_label(app, "output-rate", "Rate: No result");
    assert_label(app, "output-status", status);
    assert_label(app, "output-signature", "Backend MAC: No result");
}

fn assert_result(
    app: &FrontendApp<MemoryTransport>,
    weight: f64,
    rate: f64,
    drug_rate: f64,
    sequence: u64,
) {
    assert_eq!(app.inputs().patient_id, "demo");
    assert_eq!(app.inputs().weight_kg.to_bits(), weight.to_bits());
    assert_eq!(
        app.inputs().concentration_mg_ml.to_bits(),
        2.0_f64.to_bits()
    );
    assert_eq!(
        app.inputs().target_dose_mcg_kg_min.to_bits(),
        0.2_f64.to_bits()
    );
    assert_label(app, "label-patient", "Patient ID: demo");
    assert_label(app, "label-weight", &format!("Weight: {weight:.2} kg"));
    assert_label(app, "label-conc", "Drug Concentration: 2.00 mg/mL");
    assert_label(app, "label-dose", "Target Dose: 0.200 mcg/kg/min");
    let FormState::Success(result) = app.state() else {
        panic!("Expected backend calculation, got {:?}", app.state());
    };
    // Four rate operations, the concentration multiplication and input rounding
    // fit gamma(6); these analytic values do not come from the backend algorithm.
    let bound = 6.0 * f64::EPSILON / (1.0 - 6.0 * f64::EPSILON);
    assert!((result.rate_ml_hr - rate).abs() <= rate * bound);
    assert!((result.drug_rate_mg_hr - drug_rate).abs() <= drug_rate * bound);
    assert_eq!(result.audit_sequence_id, sequence);
    assert!(!result.is_pediatric);
    assert_label(
        app,
        "output-rate",
        &format!("Rate: {rate:.3} mL/hr ({drug_rate:.2} mg/hr)"),
    );
    assert_label(
        app,
        "output-status",
        &format!("Backend response (Audit Seq #{sequence})"),
    );
    assert_label(
        app,
        "output-signature",
        "Backend MAC: Present (not verified by frontend)",
    );
    assert_label(app, "status-badge", "SESSION ACTIVE");
}

fn capture(
    app: &FrontendApp<MemoryTransport>,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let framebuffer = app.framebuffer();
    let display = compute_layout(
        app.document(),
        i32::try_from(framebuffer.width())?,
        i32::try_from(framebuffer.height())?,
    )?;
    for command in &display.commands {
        if let DisplayCommand::DrawText {
            text, x, y, scale, ..
        } = command
        {
            let width =
                i64::try_from(text.chars().count())? * i64::from(FONT_WIDTH) * i64::from(*scale);
            let height = i64::from(FONT_HEIGHT) * i64::from(*scale);
            assert!(
                *x >= 0 && i64::from(*x) + width <= i64::from(framebuffer.width()),
                "horizontal clipping: {text}"
            );
            assert!(
                *y >= 0 && i64::from(*y) + height <= i64::from(framebuffer.height()),
                "vertical clipping: {text}"
            );
        }
    }
    write_framebuffer(framebuffer, name)
}

fn write_framebuffer(
    framebuffer: &Framebuffer,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let width = framebuffer.width();
    let height = framebuffer.height();
    let pixels = framebuffer.pixels();
    let mut bitmap = std::io::BufWriter::new(std::fs::File::create(format!("output/{name}.bmp"))?);
    // BITMAPFILEHEADER followed by 40-byte BITMAPINFOHEADER, 32-bit BI_RGB.
    let image_size = width * height * 4;
    bitmap.write_all(b"BM")?;
    bitmap.write_all(&(54 + image_size).to_le_bytes())?;
    bitmap.write_all(&[0; 4])?;
    bitmap.write_all(&54_u32.to_le_bytes())?;
    bitmap.write_all(&40_u32.to_le_bytes())?;
    bitmap.write_all(&width.to_le_bytes())?;
    bitmap.write_all(&height.to_le_bytes())?;
    bitmap.write_all(&1_u16.to_le_bytes())?;
    bitmap.write_all(&32_u16.to_le_bytes())?;
    bitmap.write_all(&0_u32.to_le_bytes())?;
    bitmap.write_all(&image_size.to_le_bytes())?;
    bitmap.write_all(&[0; 16])?;
    for row in pixels.chunks_exact(usize::try_from(width)?).rev() {
        for pixel in row {
            bitmap.write_all(&pixel.to_le_bytes())?;
        }
    }
    bitmap.flush()?;
    let mut snapshot =
        std::io::BufWriter::new(std::fs::File::create(format!("output/{name}.svg"))?);
    writeln!(
        snapshot,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" shape-rendering=\"crispEdges\">"
    )?;
    writeln!(snapshot, "<title>Metis form software framebuffer</title>")?;
    // Encode the actual raster as horizontal runs; no text or layout is rebuilt.
    for (y, row) in pixels.chunks_exact(usize::try_from(width)?).enumerate() {
        let mut x = 0;
        while let Some(&pixel) = row.get(x) {
            let length = row[x..].iter().take_while(|&&next| next == pixel).count();
            let [alpha, red, green, blue] = pixel.to_be_bytes();
            writeln!(
                snapshot,
                "<path fill=\"#{red:02x}{green:02x}{blue:02x}\" fill-opacity=\"{}\" d=\"M{x} {y}h{length}v1H{x}z\"/>",
                f64::from(alpha) / 255.0
            )?;
            x += length;
        }
    }
    writeln!(snapshot, "</svg>")?;
    snapshot.flush()?;
    Ok(())
}
