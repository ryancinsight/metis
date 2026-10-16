//! Installed-runtime owners and actual page events; no page evaluator is used.

use super::super::{
    event_loop,
    package::{Package, package_failure},
};
use crate::frontend::webview_admission::NativePage;
use metis_frontend::FormInputs;
use metis_platform::native::{
    MAX_WEBVIEW_MESSAGE_BYTES, WebViewEvent, WebViewHostEvent, WebViewSurface, WindowConfig,
    WindowEvent, WindowVisibility,
};
use metis_web::embedded::{
    PageCommand, PageEvent, PageIdentity, PageRole, PageStage, ResourceCounts, SequenceId,
    decode_page_event,
};
use std::{
    collections::VecDeque,
    io,
    time::{Duration, Instant},
};

// Condition 1: installed-runtime fixture reporting erases the actual OS,
// protocol, native browser or backend error selected by execution.
pub(super) type Failure = Box<dyn std::error::Error>;

pub(super) struct Runtime {
    surface: Option<WebViewSurface>,
    package: Option<Package>,
    pub(super) page: NativePage,
    pending: VecDeque<PageEvent>,
    deadline: Instant,
}

impl Runtime {
    pub(super) fn new(visible: bool) -> Result<Self, Failure> {
        let deadline = Instant::now() + Duration::from_mins(1);
        let package = Package::create(PageRole::Form, None)?;
        let result = (|| {
            let config = package.config()?;
            let source = config.start_uri().to_owned();
            let page = NativePage::new(package.identity(), source)?;
            let window = WindowConfig::with_visibility(
                "Metis real Rust page fixture",
                1024,
                768,
                if visible {
                    WindowVisibility::Visible
                } else {
                    WindowVisibility::Hidden
                },
            )?;
            super::super::scale::force_webview_input_scale();
            Ok::<_, io::Error>((WebViewSurface::new(&window, config)?, page))
        })();
        let (surface, page) = match result {
            Ok(value) => value,
            Err(error) => return Err(package_failure(package, error).into()),
        };
        Ok(Self {
            page,
            surface: Some(surface),
            package: Some(package),
            pending: VecDeque::new(),
            deadline,
        })
    }

    pub(super) fn next(&mut self) -> Result<PageEvent, Failure> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Ok(event);
            }
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Actual Rust page fixture exceeds 60 seconds",
                )
                .into());
            }
            let events = self
                .surface
                .as_mut()
                .expect("live surface")
                .wait_events(remaining.min(Duration::from_millis(250)))?;
            for event in events {
                match event {
                    WebViewHostEvent::WebView(WebViewEvent::Message { source, json }) => {
                        self.page.source(&source)?;
                        let event = decode_page_event(&json, MAX_WEBVIEW_MESSAGE_BYTES)?;
                        tracing::info!(event = "fixture_page_event", json = %json, source = %source, "Actual compiled-page event received");
                        self.pending.push_back(event);
                    }
                    WebViewHostEvent::WebView(WebViewEvent::NavigationCompleted {
                        success: false,
                        ..
                    }) => {
                        return Err(
                            io::Error::other("Actual compiled-page navigation fails").into()
                        );
                    }
                    WebViewHostEvent::WebView(WebViewEvent::MessageRejected { source }) => {
                        return Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            format!("Native host rejects source {source}"),
                        )
                        .into());
                    }
                    WebViewHostEvent::Window(WindowEvent::Resized { width, height })
                        if width != 0 && height != 0 =>
                    {
                        self.surface
                            .as_mut()
                            .expect("live surface")
                            .resize(width, height)?;
                    }
                    WebViewHostEvent::Window(
                        WindowEvent::Destroyed | WindowEvent::CloseRequested,
                    ) => {
                        return Err(io::Error::other(
                            "Actual window closes before fixture handshake",
                        )
                        .into());
                    }
                    _ => {}
                }
            }
        }
    }

    pub(super) fn ready(&mut self, expected: PageIdentity) -> Result<(), Failure> {
        let observed = self.next()?;
        assert_eq!(observed, PageEvent::Ready { identity: expected });
        self.page.session.ready(expected)?;
        assert_eq!(self.page.session.stage(), PageStage::Ready);
        tracing::info!(
            event = "page_ready",
            mount = expected.mount().get(),
            "Actual fixture mount is ready"
        );
        Ok(())
    }

    pub(super) fn post(&mut self, command: &PageCommand) -> Result<(), Failure> {
        event_loop::post(self.surface.as_mut().expect("live surface"), command)?;
        Ok(())
    }

    pub(super) fn genuine_submission(&mut self) -> Result<(SequenceId, FormInputs), Failure> {
        let identity = self.page.session.identity();
        // These dyadic inputs yield exactly 1 mL/h and 1.875 mg/h. The actor
        // edits the actual controls and clicks through OS input, never JS ingress.
        let expected = FormInputs::new("PT-REAL-患者", 62.5, 1.875, 0.5);
        tracing::info!(
            event = "fixture_input_required",
            mount_id = %identity.mount().get(),
            patient_id = %expected.patient_id,
            weight_kg = expected.weight_kg,
            concentration_mg_ml = expected.concentration_mg_ml,
            target_dose_mcg_kg_min = expected.target_dose_mcg_kg_min
        );
        let PageEvent::Submit {
            identity: actual,
            request,
            inputs,
        } = self.next()?
        else {
            return Err(io::Error::other("Actual OS editing produces no Rust page Submit").into());
        };
        assert_eq!(actual, identity);
        assert_eq!(request, SequenceId::new(1)?);
        assert_eq!(inputs, expected);
        self.page.session.begin_request(actual, request)?;
        Ok((request, inputs))
    }

    pub(super) fn physical_close_request(&mut self) -> Result<(), Failure> {
        let identity = self.page.session.identity();
        assert_eq!(self.next()?, PageEvent::CloseRequested { identity });
        assert_eq!(self.page.session.pending(), None);
        Ok(())
    }

    pub(super) fn malformed(&mut self, json: &str) -> Result<(), Failure> {
        self.surface
            .as_mut()
            .expect("live surface")
            .post_json(json)?;
        Ok(())
    }

    pub(super) fn failed(&mut self, context: &str) -> Result<(), Failure> {
        let PageEvent::Failed { identity, message } = self.next()? else {
            return Err(io::Error::other("Hostile native command produces no page failure").into());
        };
        assert_eq!(identity, self.page.session.identity());
        assert!(
            message.contains(context),
            "actual error {message:?} lacks {context:?}"
        );
        tracing::info!(event = "fixture_rejection", mount = identity.mount().get(), message = %message, "Actual Rust page rejects hostile native command");
        Ok(())
    }

    pub(super) fn stop(&mut self) -> Result<ResourceCounts, Failure> {
        let identity = self.page.session.identity();
        self.page.session.begin_stop(identity)?;
        self.post(&PageCommand::Stop { identity })?;
        let PageEvent::Stopped {
            identity: actual,
            resources,
        } = self.next()?
        else {
            return Err(io::Error::other("Actual page produces no stop acknowledgement").into());
        };
        assert_eq!(actual, identity);
        assert_eq!(
            resources,
            ResourceCounts {
                bridge_listeners: 1,
                ..ResourceCounts::default()
            }
        );
        self.page.session.stopped(actual, resources)?;
        assert_eq!(self.page.session.stage(), PageStage::Stopped);
        tracing::info!(
            event = "page_stopped",
            mount = identity.mount().get(),
            ?resources,
            "Actual fixture application owners released"
        );
        Ok(resources)
    }

    pub(super) fn finish(mut self) -> Result<(), Failure> {
        self.stop()?;
        event_loop::close_destroyed(self.surface.as_mut().expect("live surface"), &mut self.page)?;
        assert_eq!(self.page.session.stage(), PageStage::Closed);
        drop(self.surface.take());
        let package = self.package.take().expect("owned package");
        let root = package.root.clone();
        let profile = package.profile.clone();
        package.cleanup()?;
        assert!(!root.try_exists()?);
        assert!(!profile.try_exists()?);
        tracing::info!(
            event = "page_package_cleaned",
            root = %root.display(),
            profile = %profile.display(),
            "Actual fixture profile and package removed after owner drop"
        );
        Ok(())
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        if let Some(surface) = self.surface.as_mut()
            && let Err(error) = surface.close()
        {
            tracing::error!(%error, "Failed fixture native teardown");
        }
        drop(self.surface.take());
        if let Some(package) = self.package.take()
            && let Err(error) = package.cleanup()
        {
            tracing::error!(%error, "Failed fixture package cleanup");
        }
    }
}
