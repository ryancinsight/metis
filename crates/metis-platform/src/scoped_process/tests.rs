use super::output_reader::LIVE_OUTPUT_READERS;
use super::*;
use metis_core::capability::{CapabilityGrantSpec, CapabilityScope};
use metis_core::error::ErrorCode;
use metis_core::host::{HostOrigin, HostPolicy, HostSessionId, WindowId};
use std::io::{ErrorKind, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Instant;

const KEY: &[u8] = b"metis-scoped-process-test-key";

fn capability(
    scope: CapabilityScope,
) -> Result<VerifiedHostCapability<{ CapabilityScope::RUN_PROCESS.0 }>, metis_core::error::MetisError>
{
    let policy = HostPolicy::new(
        HostOrigin::parse("http://127.0.0.1:8080").expect("test origin"),
        WindowId::new(1).expect("test window"),
    );
    let session = HostSessionId::new([9; 16]).expect("test session");
    let context = policy.context_for(session);
    let token = context.issue_capability(
        CapabilityGrantSpec {
            token_id: 9,
            principal_id: session.as_bytes(),
            scope,
            issued_at_secs: 10,
            duration_secs: 100,
            nonce: 9,
        },
        KEY,
    )?;
    policy.authorize::<{ CapabilityScope::RUN_PROCESS.0 }>(&token, &context, 11, KEY)
}

fn fixture_provider() -> ScopedProcessProvider {
    ScopedProcessProvider::new(
        std::env::current_exe().expect("test binary"),
        ProcessContainment::DirectChild,
        [
            OsString::from("--exact"),
            OsString::from("scoped_process::tests::child_success"),
            OsString::from("scoped_process::tests::child_blocking"),
            OsString::from("scoped_process::tests::child_stderr"),
            OsString::from("--nocapture"),
        ],
    )
    .expect("provider")
    .with_environment([("METIS_SCOPED_PROCESS_CHILD", "1")])
    .expect("provider environment")
}

/// Longest a descendant holds the output pipes unless released.
const HOLDER_LIFETIME: Duration = Duration::from_secs(20);

/// Marker directory shared with the descendant that holds the output pipes,
/// removed on drop. A descendant that outlives it finds the directory gone and
/// exits without its marker.
struct Markers(PathBuf);

impl Markers {
    fn new(label: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "metis-scoped-process-{label}-{}",
            std::process::id()
        ));
        match fs::remove_dir_all(&directory) {
            Ok(()) => {}
            // No earlier process with this identifier left one behind.
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => panic!("stale marker directory: {error}"),
        }
        fs::create_dir_all(&directory).expect("marker directory");
        Self(directory)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Markers {
    fn drop(&mut self) {
        // A surviving descendant can write its marker between the scan and the
        // removal of the directory, so the removal is tried twice. A directory
        // that still remains is cleared by the next `Markers::new` with this
        // label and process identifier, and a destructor must not panic.
        for _ in 0..2 {
            match fs::remove_dir_all(&self.0) {
                Ok(()) => return,
                Err(error) if error.kind() == ErrorKind::NotFound => return,
                Err(_contended) => {}
            }
        }
    }
}

fn holder_provider(markers: &Path) -> ScopedProcessProvider {
    let program = std::env::current_exe().expect("test binary");
    ScopedProcessProvider::new(
        program,
        ProcessContainment::DirectChild,
        [
            OsString::from("--exact"),
            OsString::from("scoped_process::tests::child_exits_behind_holder"),
            OsString::from("scoped_process::tests::child_blocks_behind_holder"),
            OsString::from("--nocapture"),
        ],
    )
    .expect("provider")
    .with_environment([
        ("METIS_SCOPED_PROCESS_CHILD", OsString::from("1")),
        (
            "METIS_SCOPED_PROCESS_MARKERS",
            markers.as_os_str().to_owned(),
        ),
    ])
    .expect("provider environment")
}

#[expect(
    clippy::zombie_processes,
    reason = "The descendant outlives its parent by design and exits on the release marker"
)]
fn spawn_holder() {
    std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "scoped_process::tests::child_holder",
            "--nocapture",
        ])
        .spawn()
        .expect("holder spawn");
}

fn child_invocation() -> bool {
    std::env::var_os("METIS_SCOPED_PROCESS_CHILD").is_some()
}

#[test]
fn child_success() {
    if child_invocation() {
        std::io::stdout()
            .write_all(b"process-output")
            .expect("child stdout");
    }
}

#[test]
fn child_stderr() {
    if child_invocation() {
        std::io::stderr()
            .write_all(b"secret-token")
            .expect("child stderr");
    }
}

#[test]
fn child_blocking() {
    if child_invocation() {
        let mut byte = [0u8; 1];
        std::io::stdin()
            .read_exact(&mut byte)
            .expect("parent closes stdin at cleanup");
    }
}

#[test]
fn runs_allowlisted_process_and_redacts_stderr_content() {
    let capability = capability(CapabilityScope::RUN_PROCESS).expect("capability");
    let provider = fixture_provider();
    let output = provider
        .run(
            &capability,
            [
                "--exact",
                "scoped_process::tests::child_stderr",
                "--nocapture",
            ],
            Duration::from_secs(5),
        )
        .expect("process output");
    assert!(!String::from_utf8_lossy(output.stdout()).contains("secret-token"));
    assert_eq!(output.stderr_bytes(), b"secret-token".len());
    assert_eq!(output.outcome(), ProcessOutcome::Succeeded);
    assert!(!format!("{output:?}").contains("secret-token"));
}

#[test]
fn returns_real_stdout_and_rejects_invalid_deadline_or_program() {
    let capability = capability(CapabilityScope::RUN_PROCESS).expect("capability");
    let provider = fixture_provider();
    let output = provider
        .run(
            &capability,
            [
                "--exact",
                "scoped_process::tests::child_success",
                "--nocapture",
            ],
            Duration::from_secs(5),
        )
        .expect("process output");
    assert!(String::from_utf8_lossy(output.stdout()).contains("process-output"));
    assert_eq!(output.outcome(), ProcessOutcome::Succeeded);
    assert!(matches!(
        provider.run(&capability, std::iter::empty::<OsString>(), Duration::ZERO,),
        Err(ScopedProcessError::InvalidDeadline)
    ));
    assert!(matches!(
        ScopedProcessProvider::new(
            "relative.exe",
            ProcessContainment::DirectChild,
            std::iter::empty::<OsString>(),
        ),
        Err(ScopedProcessError::InvalidProgram)
    ));
}

#[test]
fn rejects_arguments_and_missing_scope() {
    let provider = fixture_provider();
    let process_capability = capability(CapabilityScope::RUN_PROCESS).expect("capability");
    assert!(matches!(
        provider.run(
            &process_capability,
            ["--unsupported"],
            Duration::from_secs(1)
        ),
        Err(ScopedProcessError::ArgumentDenied)
    ));
    let denied = capability(CapabilityScope::UI_RENDER).expect_err("scope denial");
    assert_eq!(denied.code, ErrorCode::InsufficientScope);
}

#[test]
fn deadline_terminates_a_real_child() {
    let capability = capability(CapabilityScope::RUN_PROCESS).expect("capability");
    let provider = fixture_provider();
    let error = provider
        .run(
            &capability,
            [
                "--exact",
                "scoped_process::tests::child_blocking",
                "--nocapture",
            ],
            Duration::from_millis(10),
        )
        .expect_err("deadline");
    assert!(matches!(error, ScopedProcessError::DeadlineExceeded));
}

#[test]
fn child_holder() {
    if let Some(markers) = std::env::var_os("METIS_SCOPED_PROCESS_MARKERS") {
        let markers = PathBuf::from(markers);
        let started = Instant::now();
        while !markers.join("release").exists() && started.elapsed() < HOLDER_LIFETIME {
            std::thread::park_timeout(Duration::from_millis(25));
        }
        // The test may have finished and removed the directory already.
        match fs::write(markers.join("exited"), b"") {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => panic!("exit marker: {error}"),
        }
    }
}

#[test]
fn child_exits_behind_holder() {
    if child_invocation() {
        spawn_holder();
        if let Some(markers) = std::env::var_os("METIS_SCOPED_PROCESS_MARKERS") {
            fs::write(PathBuf::from(markers).join("child_done"), b"").expect("child marker");
        }
    }
}

#[test]
fn child_blocks_behind_holder() {
    if child_invocation() {
        spawn_holder();
        let mut byte = [0u8; 1];
        std::io::stdin()
            .read_exact(&mut byte)
            .expect("parent closes stdin at cleanup");
    }
}

/// Runs `fixture` under `deadline`; returns whether the error was the deadline
/// and whether the descendant holding the output pipes had exited by the time
/// `run` returned.
fn run_behind_holder(markers: &Markers, fixture: &str, deadline: Duration) -> (bool, bool) {
    let capability = capability(CapabilityScope::RUN_PROCESS).expect("capability");
    let error = holder_provider(markers.path())
        .run(&capability, ["--exact", fixture, "--nocapture"], deadline)
        .expect_err("deadline");
    let holder_exited = markers.path().join("exited").exists();
    fs::write(markers.path().join("release"), b"").expect("release marker");
    (
        matches!(error, ScopedProcessError::DeadlineExceeded),
        holder_exited,
    )
}

#[test]
fn exited_child_does_not_wait_for_a_descendant_holding_stdout() {
    let markers = Markers::new("exited");
    // Long enough for the child to start and exit on a loaded runner, so the
    // deadline that fires is the one on the output pipes.
    let (deadline_exceeded, holder_exited) = run_behind_holder(
        &markers,
        "scoped_process::tests::child_exits_behind_holder",
        Duration::from_secs(5),
    );
    assert!(
        markers.path().join("child_done").exists(),
        "the child must exit before the deadline so that only the output pipes are left"
    );
    assert!(deadline_exceeded);
    assert!(
        !holder_exited,
        "run returned only after the descendant released the output pipe"
    );
}

#[test]
fn deadline_does_not_wait_for_a_descendant_holding_stdout() {
    let markers = Markers::new("blocked");
    let (deadline_exceeded, holder_exited) = run_behind_holder(
        &markers,
        "scoped_process::tests::child_blocks_behind_holder",
        Duration::from_millis(500),
    );
    assert!(deadline_exceeded);
    assert!(
        !holder_exited,
        "run returned only after the descendant released the output pipe"
    );
}

#[test]
fn a_refused_run_starts_no_process() {
    // A regular file that cannot be executed: a run that reached the spawn
    // would fail there, not at the reader bound.
    let markers = Markers::new("refused");
    let program = markers.path().join("not-a-program");
    fs::write(&program, b"text").expect("program file");
    let provider = ScopedProcessProvider::new(
        &program,
        ProcessContainment::DirectChild,
        std::iter::empty::<OsString>(),
    )
    .expect("provider");
    let held: Vec<ReaderSlot> = (0..MAX_SCOPED_PROCESS_OUTPUT_READERS)
        .map(|_| ReaderSlot::acquire().expect("slot below the bound"))
        .collect();
    let capability = capability(CapabilityScope::RUN_PROCESS).expect("capability");
    let error = provider
        .run(
            &capability,
            std::iter::empty::<OsString>(),
            Duration::from_secs(5),
        )
        .expect_err("reader bound");
    drop(held);
    assert!(
        matches!(error, ScopedProcessError::OutputReadersExhausted),
        "{error:?}"
    );
    let error = provider
        .run(
            &capability,
            std::iter::empty::<OsString>(),
            Duration::from_secs(5),
        )
        .expect_err("a file that cannot be executed");
    assert!(matches!(error, ScopedProcessError::Process(_)), "{error:?}");
}

#[test]
fn collect_waits_only_for_the_time_left_of_the_deadline() {
    let (sender, result) = mpsc::sync_channel::<Result<(), ScopedProcessError>>(1);
    let reader = OutputReader { result };
    let deadline = Duration::from_secs(5);
    // The run began four seconds ago, so one second of the deadline remains.
    let started = Instant::now()
        .checked_sub(Duration::from_secs(4))
        .expect("a clock more than four seconds past its origin");
    let waiting = Instant::now();
    let error = reader
        .collect(started, deadline)
        .expect_err("no output arrives");
    let waited = waiting.elapsed();
    drop(sender);
    assert!(matches!(error, ScopedProcessError::DeadlineExceeded));
    // `recv_timeout` never returns early, and just under one second remains.
    assert!(
        (Duration::from_millis(500)..Duration::from_secs(3)).contains(&waited),
        "waited {waited:?} of a deadline with one second left"
    );
}

#[test]
fn reader_slots_are_bounded_and_released() {
    let mut held = Vec::new();
    for _ in 0..MAX_SCOPED_PROCESS_OUTPUT_READERS {
        held.push(ReaderSlot::acquire().expect("slot below the bound"));
    }
    assert!(matches!(
        ReaderSlot::acquire(),
        Err(ScopedProcessError::OutputReadersExhausted)
    ));
    held.pop();
    held.push(ReaderSlot::acquire().expect("a released slot is reusable"));
    held.clear();
    assert_eq!(LIVE_OUTPUT_READERS.load(Ordering::Relaxed), 0);
}

#[test]
fn output_exactly_at_the_bound_is_accepted_and_one_byte_more_is_not() {
    let bound = u64::try_from(MAX_SCOPED_PROCESS_OUTPUT_BYTES).expect("bound fits u64");
    let at_bound = std::io::repeat(b'x').take(bound);
    assert_eq!(
        drain_bounded(at_bound, |_| Ok(())).expect("output at the bound"),
        MAX_SCOPED_PROCESS_OUTPUT_BYTES
    );
    let over = std::io::repeat(b'x').take(bound + 1);
    assert!(matches!(
        drain_bounded(over, |_| Ok(())),
        Err(ScopedProcessError::OutputTooLarge)
    ));
}

#[test]
fn output_beyond_the_bound_fails_without_retaining_it() {
    let (reader, mut writer) = std::io::pipe().expect("pipe");
    let writer_thread = std::thread::spawn(move || {
        let chunk = [b'x'; 4096];
        for _ in 0..=MAX_SCOPED_PROCESS_OUTPUT_BYTES / chunk.len() {
            if writer.write_all(&chunk).is_err() {
                break;
            }
        }
    });
    let mut retained = 0usize;
    let error = drain_bounded(reader, |chunk| {
        retained += chunk.len();
        Ok(())
    })
    .expect_err("bound");
    writer_thread.join().expect("writer");
    assert!(matches!(error, ScopedProcessError::OutputTooLarge));
    assert!(retained <= MAX_SCOPED_PROCESS_OUTPUT_BYTES);
}
