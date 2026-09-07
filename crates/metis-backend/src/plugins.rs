//! Bounded host-side dispatch for permission-scoped plugin operations.

use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{
    MAX_PLUGINS, Plugin, PluginDescriptor, PluginInvocationPayload,
    PluginInvocationResponsePayload, PluginOperation, PluginRegistry,
};

/// Executes the body of one statically registered plugin operation.
///
/// The plugin owns its body codec. It must reject malformed bytes with a typed
/// error and return a response body that the router can bound before encoding.
pub trait PluginExecutor: Send {
    /// Dispatches one exact operation name and its opaque body.
    ///
    /// # Errors
    /// Returns a typed plugin or domain error when the body is invalid or the
    /// operation cannot complete.
    fn invoke(&mut self, operation_name: &str, body: &[u8]) -> Result<Vec<u8>>;
}

struct RegisteredPlugin {
    descriptor: PluginDescriptor,
    // The plugin set is open at this extension boundary. Dynamic dispatch stays
    // here; clinical calculation and frame codec paths remain static.
    executor: Box<dyn PluginExecutor>,
}

/// Bounded runtime registry for permission-checked remote plugin calls.
pub(crate) struct PluginRouter<const CAPACITY: usize = MAX_PLUGINS> {
    metadata: PluginRegistry<CAPACITY>,
    executors: Vec<RegisteredPlugin>,
}

impl<const CAPACITY: usize> PluginRouter<CAPACITY> {
    /// Creates an empty router with a bounded plugin capacity.
    ///
    /// # Errors
    /// Returns the registry's typed capacity error when `CAPACITY` is outside
    /// the host bound.
    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            metadata: PluginRegistry::new()?,
            executors: Vec::new(),
        })
    }

    /// Registers one statically described plugin and its executor.
    ///
    /// Registration validates identifiers, operation scopes, duplicate names
    /// and total capacity before the executor becomes reachable remotely.
    ///
    /// # Errors
    /// Returns a typed descriptor, duplicate, capacity or allocation error.
    pub(crate) fn register<P>(&mut self, executor: P) -> Result<()>
    where
        P: Plugin + PluginExecutor + 'static,
    {
        self.executors.try_reserve_exact(1).map_err(|_| {
            MetisError::capability(
                ErrorCode::PluginRegistryFull,
                "Plugin executor allocation failed within its bound",
            )
        })?;
        self.metadata.register::<P>()?;
        self.executors.push(RegisteredPlugin {
            descriptor: P::DESCRIPTOR,
            executor: Box::new(executor),
        });
        Ok(())
    }

    /// Returns a registered command used for remote invocation authorization.
    #[must_use]
    pub(crate) fn command(&self, plugin_name: &str, command_name: &str) -> Option<PluginOperation> {
        self.metadata.command(plugin_name, command_name)
    }

    /// Returns whether the exact plugin identifier is registered.
    #[must_use]
    pub(crate) fn contains(&self, plugin_name: &str) -> bool {
        self.metadata.get(plugin_name).is_some()
    }

    /// Invokes an exact registered command after the caller authorizes its scope.
    ///
    /// The router does not inspect or reinterpret the plugin body. Callers
    /// must authorize [`PluginOperation::required_scope`] against the trusted
    /// session token before calling this method.
    ///
    /// # Errors
    /// Returns `PluginNotFound`, `PluginOperationNotFound`, a response-size
    /// error, or the executor's typed error.
    pub(crate) fn invoke(
        &mut self,
        request: &PluginInvocationPayload,
    ) -> Result<PluginInvocationResponsePayload> {
        if self.metadata.get(request.plugin_name()).is_none() {
            return Err(MetisError::capability(
                ErrorCode::PluginNotFound,
                "Plugin invocation names an unregistered plugin",
            ));
        }
        let Some(operation) = self.command(request.plugin_name(), request.operation_name()) else {
            return Err(MetisError::capability(
                ErrorCode::PluginOperationNotFound,
                "Plugin invocation names an undeclared operation",
            ));
        };
        let Some(registered) = self
            .executors
            .iter_mut()
            .find(|entry| entry.descriptor.name() == request.plugin_name())
        else {
            return Err(MetisError::new(
                ErrorCode::InvalidPluginDescriptor,
                "Plugin metadata has no matching executor",
                "REQ-METIS-SEC-002",
            ));
        };
        let response = registered
            .executor
            .invoke(operation.name(), request.body())?;
        PluginInvocationResponsePayload::new(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use metis_core::capability::{CapabilityScope, CapabilityToken};
    use metis_core::protocol::{PluginDescriptor, PluginOperation};

    static COMMANDS: [PluginOperation; 1] = [PluginOperation::new(
        "echo",
        CapabilityScope::SUBMIT_CALCULATION,
    )];
    static EVENTS: [PluginOperation; 1] = [PluginOperation::new(
        "changed",
        CapabilityScope::STREAM_TELEMETRY,
    )];

    struct EchoPlugin;

    impl Plugin for EchoPlugin {
        const DESCRIPTOR: PluginDescriptor = PluginDescriptor::new("echo", 1, &COMMANDS, &EVENTS);
    }

    impl PluginExecutor for EchoPlugin {
        fn invoke(&mut self, operation_name: &str, body: &[u8]) -> Result<Vec<u8>> {
            if operation_name != "echo" {
                return Err(MetisError::capability(
                    ErrorCode::PluginOperationNotFound,
                    "Test executor received an undeclared operation",
                ));
            }
            Ok(body.iter().map(|byte| byte.wrapping_add(1)).collect())
        }
    }

    fn request(plugin: &str, operation: &str, body: &[u8]) -> PluginInvocationPayload {
        PluginInvocationPayload::new(
            CapabilityToken::issue(
                1,
                [1; 16],
                CapabilityScope::SUBMIT_CALCULATION,
                1,
                10,
                1,
                b"key",
            ),
            plugin,
            operation,
            body,
        )
        .expect("invocation")
    }

    #[test]
    fn router_dispatches_input_sensitive_executor_output() {
        let mut router = PluginRouter::<1>::new().expect("router");
        router.register(EchoPlugin).expect("register");
        assert_eq!(
            router
                .invoke(&request("echo", "echo", [1, 4, 9].as_slice()))
                .expect("invoke")
                .body(),
            [2, 5, 10]
        );
    }

    #[test]
    fn router_rejects_unknown_plugin_and_operation() {
        let mut router = PluginRouter::<1>::new().expect("router");
        router.register(EchoPlugin).expect("register");
        assert_eq!(
            router
                .invoke(&request("missing", "echo", b"body"))
                .expect_err("unknown plugin")
                .code,
            ErrorCode::PluginNotFound
        );
        assert_eq!(
            router
                .invoke(&request("echo", "missing", b"body"))
                .expect_err("unknown operation")
                .code,
            ErrorCode::PluginOperationNotFound
        );
        assert_eq!(
            router
                .invoke(&request("echo", "changed", b"body"))
                .expect_err("event is not invokable")
                .code,
            ErrorCode::PluginOperationNotFound
        );
    }
}
