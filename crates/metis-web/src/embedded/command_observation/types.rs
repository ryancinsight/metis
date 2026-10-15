//! Domain observations retain native values without serialized public DTOs.

use crate::embedded::PageStage;
use metis_frontend::ApplicationCommand;

/// One actual keyboard metadata snapshot, without truncation or interpretation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Six independent observed KeyboardEvent flags admit all combinations; they are not alternative control states"
)]
pub struct KeyboardSnapshot {
    /// Browser semantic key, including its original Unicode spelling.
    pub key: String,
    /// Browser physical code, kept separately from the semantic key.
    pub code: String,
    /// Observed Control modifier.
    pub ctrl: bool,
    /// Observed Alt modifier.
    pub alt: bool,
    /// Observed Shift modifier.
    pub shift: bool,
    /// Observed Meta modifier.
    pub meta: bool,
    /// Browser repeat flag.
    pub repeat: bool,
    /// Browser trusted-event flag; this observation grants no authority.
    pub trusted: bool,
}

/// Admission result before command interpretation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Admission {
    /// Readiness and actual keyboard metadata both admit interpretation.
    Ready,
    /// The page lifecycle rejects interpretation before metadata is read.
    NotReady,
    /// The actual event is not a keyboard event.
    NotKeyboard,
    /// The provider returns an error while reading keyboard metadata.
    MetadataError,
}

/// Actual command-policy decision, separate from input receipt and execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Decision {
    /// Admission prevents command interpretation.
    NotEvaluated,
    /// The repeat policy ignores this keyboard event.
    Repeat,
    /// Escape closes the command menu.
    EscapeMenu,
    /// Escape requests native page closure.
    EscapeClose,
    /// The shared command catalogue selects this application command.
    Command(ApplicationCommand),
    /// Interpretation selects no command.
    Ignored,
}

/// Actual return classification from one command listener invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// The observed listener returns successfully.
    Ok,
    /// Readiness rejects the invocation.
    ReadyRejected,
    /// The provider fails while reading keyboard metadata.
    MetadataError,
    /// A selected application action returns an error.
    ActionError,
}

/// One closed command observation event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    /// The exact opt-in marker and provider selection enable this stream.
    Selected,
    /// The command listener enters with the current page stage.
    Entered {
        /// Actual lifecycle stage before readiness admission.
        stage: PageStage,
    },
    /// Admission and interpretation preserve the actual metadata snapshot.
    MetadataDecision {
        /// Present exactly when keyboard metadata admits interpretation.
        keyboard: Option<KeyboardSnapshot>,
        /// Actual readiness/provider admission.
        admission: Admission,
        /// Actual policy decision; no action success is implied.
        decision: Decision,
    },
    /// The listener reaches its return boundary.
    Returned {
        /// Actual return classification.
        outcome: Outcome,
    },
    /// The observer intends to close; this does not prove native destruction.
    ClosingIntent,
    /// Further ordinary observation cannot fit the reserved stream budget.
    Quota {
        /// Producer records admitted before the quota notice.
        prior_records: u32,
        /// Producer encoded bytes admitted before the quota notice.
        prior_bytes: u32,
        /// Exact omission count at this notice, transported as decimal text.
        omitted_at_notice: u64,
        /// The producer has also observed a recording failure.
        failed: bool,
    },
}
