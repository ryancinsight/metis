//! How much motion a host asks a presentation to show.

/// Whether a declared transition is sampled over its duration or applied at
/// once.
///
/// The platform is the only party that knows the operating-system setting, so
/// it supplies this value. A host that cannot read the setting reports
/// [`Self::Full`], which paints exactly what a document that declares no
/// transition paints; [`Self::Reduced`] paints the target value in the frame
/// the change lands in, so a state change is never delayed by an animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MotionPreference {
    /// Sample a declared transition over its duration.
    #[default]
    Full,
    /// Apply the target value in one frame, as a request to reduce motion
    /// requires.
    Reduced,
}
