//! Bounded, typed plugin metadata registration.

use crate::capability::CapabilityScope;
use crate::error::{ErrorCode, MetisError, Result};

/// Maximum number of registered plugin manifests in one host.
pub const MAX_PLUGINS: usize = 16;
/// Maximum number of commands or events declared by one plugin.
pub const MAX_PLUGIN_OPERATIONS: usize = 16;
/// Maximum UTF-8 byte length of a plugin identifier.
pub const MAX_PLUGIN_NAME_BYTES: usize = 64;
/// Maximum UTF-8 byte length of a plugin operation identifier.
pub const MAX_PLUGIN_OPERATION_NAME_BYTES: usize = 128;

/// Metadata for one plugin command or event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginOperation {
    name: &'static str,
    required_scope: CapabilityScope,
}

impl PluginOperation {
    /// Creates a statically declared operation descriptor.
    #[must_use]
    pub const fn new(name: &'static str, required_scope: CapabilityScope) -> Self {
        Self {
            name,
            required_scope,
        }
    }

    /// Returns the stable operation identifier.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the capability scope required by this operation.
    #[must_use]
    pub const fn required_scope(self) -> CapabilityScope {
        self.required_scope
    }
}

/// Statically declared plugin metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginDescriptor {
    name: &'static str,
    version: u16,
    commands: &'static [PluginOperation],
    events: &'static [PluginOperation],
}

impl PluginDescriptor {
    /// Creates metadata for a typed plugin.
    #[must_use]
    pub const fn new(
        name: &'static str,
        version: u16,
        commands: &'static [PluginOperation],
        events: &'static [PluginOperation],
    ) -> Self {
        Self {
            name,
            version,
            commands,
            events,
        }
    }

    /// Returns the stable plugin identifier.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the plugin contract version.
    #[must_use]
    pub const fn version(self) -> u16 {
        self.version
    }

    /// Returns the declared command operations.
    #[must_use]
    pub const fn commands(self) -> &'static [PluginOperation] {
        self.commands
    }

    /// Returns the declared event operations.
    #[must_use]
    pub const fn events(self) -> &'static [PluginOperation] {
        self.events
    }
}

/// Compile-time plugin seam for host registration.
pub trait Plugin {
    /// The complete, statically declared plugin metadata.
    const DESCRIPTOR: PluginDescriptor;
}

/// Bounded host-local registry for typed plugin manifests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRegistry<const CAPACITY: usize = MAX_PLUGINS> {
    descriptors: Vec<PluginDescriptor>,
}

impl<const CAPACITY: usize> PluginRegistry<CAPACITY> {
    /// Creates an empty registry with a bounded capacity.
    ///
    /// # Errors
    /// Returns [`ErrorCode::PluginRegistryFull`] for zero capacity or
    /// [`ErrorCode::PayloadTooLarge`] when the requested capacity exceeds the
    /// host bound.
    pub fn new() -> Result<Self> {
        if CAPACITY == 0 {
            return Err(MetisError::capability(
                ErrorCode::PluginRegistryFull,
                "Plugin registry capacity must be non-zero",
            ));
        }
        if CAPACITY > MAX_PLUGINS {
            return Err(MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Plugin registry capacity exceeds the host bound",
            ));
        }
        Ok(Self {
            descriptors: Vec::new(),
        })
    }

    /// Registers a typed plugin manifest exactly once.
    ///
    /// Registration stores static metadata and does not erase a plugin
    /// handler or grant operating-system authority. Runtime command/event
    /// handling remains at the host boundary, where capability checks are
    /// applied before dispatch.
    ///
    /// # Errors
    /// Returns a typed descriptor, duplicate, capacity, or allocation error.
    pub fn register<P: Plugin>(&mut self) -> Result<()> {
        let descriptor = P::DESCRIPTOR;
        validate_descriptor(descriptor)?;
        if self
            .descriptors
            .iter()
            .any(|registered| registered.name == descriptor.name)
        {
            return Err(MetisError::capability(
                ErrorCode::PluginAlreadyRegistered,
                "Plugin identifier is already registered",
            ));
        }
        if self.descriptors.len() >= CAPACITY {
            return Err(MetisError::capability(
                ErrorCode::PluginRegistryFull,
                "Plugin registry capacity is full",
            ));
        }
        self.descriptors.try_reserve_exact(1).map_err(|_| {
            MetisError::capability(
                ErrorCode::PluginRegistryFull,
                "Plugin registry allocation failed within its bound",
            )
        })?;
        self.descriptors.push(descriptor);
        Ok(())
    }

    /// Returns a registered plugin by its exact identifier.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&PluginDescriptor> {
        self.descriptors
            .iter()
            .find(|descriptor| descriptor.name == name)
    }

    /// Returns a registered operation by plugin and exact operation name.
    #[must_use]
    pub fn operation(&self, plugin_name: &str, operation_name: &str) -> Option<PluginOperation> {
        let descriptor = self.get(plugin_name)?;
        descriptor
            .commands
            .iter()
            .chain(descriptor.events)
            .copied()
            .find(|operation| operation.name == operation_name)
    }

    /// Returns a registered command by plugin and exact command name.
    #[must_use]
    pub fn command(&self, plugin_name: &str, command_name: &str) -> Option<PluginOperation> {
        let descriptor = self.get(plugin_name)?;
        descriptor
            .commands
            .iter()
            .copied()
            .find(|operation| operation.name == command_name)
    }

    /// Returns all registered manifests in registration order.
    #[must_use]
    pub fn plugins(&self) -> &[PluginDescriptor] {
        &self.descriptors
    }

    /// Returns the number of registered manifests.
    #[must_use]
    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    /// Returns whether the registry contains no manifests.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }
}

fn validate_descriptor(descriptor: PluginDescriptor) -> Result<()> {
    if !valid_identifier(descriptor.name, MAX_PLUGIN_NAME_BYTES) || descriptor.version == 0 {
        return Err(MetisError::capability(
            ErrorCode::InvalidPluginDescriptor,
            "Plugin identifier or version is invalid",
        ));
    }
    if descriptor.commands.len() > MAX_PLUGIN_OPERATIONS
        || descriptor.events.len() > MAX_PLUGIN_OPERATIONS
        || descriptor
            .commands
            .len()
            .checked_add(descriptor.events.len())
            .is_none_or(|count| count > MAX_PLUGIN_OPERATIONS)
    {
        return Err(MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Plugin operation count exceeds the host bound",
        ));
    }
    for (index, operation) in descriptor
        .commands
        .iter()
        .chain(descriptor.events)
        .enumerate()
    {
        if !valid_identifier(operation.name, MAX_PLUGIN_OPERATION_NAME_BYTES)
            || operation.required_scope == CapabilityScope::NONE
        {
            return Err(MetisError::capability(
                ErrorCode::InvalidPluginDescriptor,
                "Plugin operation identifier or capability scope is invalid",
            ));
        }
        if descriptor
            .commands
            .iter()
            .chain(descriptor.events)
            .take(index)
            .any(|previous| previous.name == operation.name)
        {
            return Err(MetisError::capability(
                ErrorCode::InvalidPluginDescriptor,
                "Plugin operation identifiers must be unique",
            ));
        }
    }
    Ok(())
}

pub(super) fn valid_identifier(value: &str, max_bytes: usize) -> bool {
    let Some(first) = value.as_bytes().first().copied() else {
        return false;
    };
    value.len() <= max_bytes
        && first.is_ascii_lowercase()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    static COMMANDS: [PluginOperation; 1] =
        [PluginOperation::new("open", CapabilityScope::UI_RENDER)];
    static EVENTS: [PluginOperation; 1] = [PluginOperation::new(
        "opened",
        CapabilityScope::STREAM_TELEMETRY,
    )];

    struct ViewerPlugin;

    impl Plugin for ViewerPlugin {
        const DESCRIPTOR: PluginDescriptor = PluginDescriptor::new("viewer", 1, &COMMANDS, &EVENTS);
    }

    struct OtherPlugin;

    impl Plugin for OtherPlugin {
        const DESCRIPTOR: PluginDescriptor = PluginDescriptor::new("other", 2, &[], &[]);
    }

    #[test]
    fn registration_preserves_typed_manifest_and_operation_scope() {
        let mut registry = PluginRegistry::<2>::new().expect("registry");
        assert!(registry.is_empty());
        registry.register::<ViewerPlugin>().expect("register");
        let descriptor = registry.get("viewer").expect("viewer manifest");
        assert_eq!(descriptor.version(), 1);
        assert_eq!(
            descriptor.commands()[0].required_scope(),
            CapabilityScope::UI_RENDER
        );
        assert_eq!(
            registry
                .operation("viewer", "opened")
                .expect("event operation")
                .required_scope(),
            CapabilityScope::STREAM_TELEMETRY
        );
        assert!(registry.command("viewer", "opened").is_none());
        assert_eq!(
            registry
                .command("viewer", "open")
                .expect("command operation")
                .required_scope(),
            CapabilityScope::UI_RENDER
        );
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn duplicate_and_capacity_limits_are_explicit() {
        let mut registry = PluginRegistry::<1>::new().expect("registry");
        registry.register::<ViewerPlugin>().expect("register");
        assert_eq!(
            registry
                .register::<ViewerPlugin>()
                .expect_err("duplicate")
                .code,
            ErrorCode::PluginAlreadyRegistered
        );
        assert_eq!(
            registry
                .register::<OtherPlugin>()
                .expect_err("capacity")
                .code,
            ErrorCode::PluginRegistryFull
        );
    }

    #[test]
    fn malformed_manifest_fields_are_rejected() {
        static EMPTY_SCOPE: [PluginOperation; 1] =
            [PluginOperation::new("open", CapabilityScope::NONE)];
        static DUPLICATE: [PluginOperation; 2] = [
            PluginOperation::new("open", CapabilityScope::UI_RENDER),
            PluginOperation::new("open", CapabilityScope::UI_RENDER),
        ];
        static TOO_MANY: [PluginOperation; MAX_PLUGIN_OPERATIONS + 1] =
            [PluginOperation::new("open", CapabilityScope::UI_RENDER); MAX_PLUGIN_OPERATIONS + 1];
        struct EmptyScope;
        impl Plugin for EmptyScope {
            const DESCRIPTOR: PluginDescriptor =
                PluginDescriptor::new("empty-scope", 1, &EMPTY_SCOPE, &[]);
        }
        struct Duplicate;
        impl Plugin for Duplicate {
            const DESCRIPTOR: PluginDescriptor =
                PluginDescriptor::new("duplicate", 1, &DUPLICATE, &[]);
        }
        struct TooMany;
        impl Plugin for TooMany {
            const DESCRIPTOR: PluginDescriptor =
                PluginDescriptor::new("too-many", 1, &TOO_MANY, &[]);
        }
        struct InvalidName;
        impl Plugin for InvalidName {
            const DESCRIPTOR: PluginDescriptor = PluginDescriptor::new("Invalid", 1, &[], &[]);
        }
        struct InvalidVersion;
        impl Plugin for InvalidVersion {
            const DESCRIPTOR: PluginDescriptor =
                PluginDescriptor::new("invalid-version", 0, &[], &[]);
        }
        let mut registry = PluginRegistry::<MAX_PLUGINS>::new().expect("registry");
        assert_eq!(
            registry
                .register::<EmptyScope>()
                .expect_err("empty scope")
                .code,
            ErrorCode::InvalidPluginDescriptor
        );
        assert_eq!(
            registry
                .register::<Duplicate>()
                .expect_err("duplicate operation")
                .code,
            ErrorCode::InvalidPluginDescriptor
        );
        assert_eq!(
            registry
                .register::<InvalidName>()
                .expect_err("invalid name")
                .code,
            ErrorCode::InvalidPluginDescriptor
        );
        assert_eq!(
            registry
                .register::<InvalidVersion>()
                .expect_err("invalid version")
                .code,
            ErrorCode::InvalidPluginDescriptor
        );
        assert_eq!(
            registry
                .register::<TooMany>()
                .expect_err("operation bound")
                .code,
            ErrorCode::PayloadTooLarge
        );
    }

    #[test]
    fn registry_capacity_parameter_is_bounded() {
        assert_eq!(
            PluginRegistry::<0>::new().expect_err("zero capacity").code,
            ErrorCode::PluginRegistryFull
        );
        assert_eq!(
            PluginRegistry::<{ MAX_PLUGINS + 1 }>::new()
                .expect_err("oversized capacity")
                .code,
            ErrorCode::PayloadTooLarge
        );
    }
}
