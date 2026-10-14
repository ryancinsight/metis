//! Native window and SDK policy events preserve the shared lifecycle owner.
use super::{ESCAPE_KEY, State, post, protocol};
use crate::frontend::webview::requests;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_frontend::FrontendApp;
use metis_ipc::IpcTransport;
use metis_platform::native::{
    WebViewEvent, WebViewHostEvent, WebViewPermission, WebViewSurface, WindowEvent,
};
use metis_web::embedded::{PageCapability, PageCommand, PageRole, PageStage};

impl State {
    pub(super) fn dispatch<T: IpcTransport>(
        &mut self,
        app: &mut FrontendApp<T>,
        surface: &mut WebViewSurface,
        event: WebViewHostEvent,
    ) -> Result<bool> {
        match event {
            WebViewHostEvent::Window(
                WindowEvent::CloseRequested
                | WindowEvent::KeyDown {
                    virtual_key: ESCAPE_KEY,
                    ..
                },
            ) => self.begin_stop(surface)?,
            WebViewHostEvent::Window(WindowEvent::Destroyed) => {
                self.page
                    .session
                    .observe_destroyed()
                    .map_err(|error| protocol(&error))?;
                return Ok(true);
            }
            WebViewHostEvent::Window(WindowEvent::Resized { width, height }) => {
                // Minimized windows have no renderable client rectangle;
                // restore supplies the next nonempty native size.
                if width != 0 && height != 0 {
                    surface.resize(width, height)?;
                }
            }
            WebViewHostEvent::WebView(WebViewEvent::Message { source, json }) => {
                return self.message(app, surface, &source, &json);
            }
            WebViewHostEvent::WebView(WebViewEvent::PermissionDenied {
                uri,
                permission,
                user_initiated,
            }) => self.permission_denied(surface, &uri, permission, user_initiated)?,
            WebViewHostEvent::WebView(WebViewEvent::NavigationStarting { uri, allowed }) => {
                tracing::info!(
                    event = "page_navigation_starting",
                    uri,
                    allowed,
                    mount = self.page.session.identity().mount().get(),
                    "Actual packaged page navigation policy observed"
                );
            }
            WebViewHostEvent::WebView(WebViewEvent::NavigationCompleted { uri, success }) => {
                tracing::info!(
                    event = "page_navigation_completed",
                    uri,
                    success,
                    mount = self.page.session.identity().mount().get(),
                    "Actual packaged page document navigation completed"
                );
                if !success {
                    return Err(MetisError::protocol(
                        ErrorCode::NavigationDenied,
                        format!("Packaged page navigation failed: {uri}"),
                    ));
                }
            }
            WebViewHostEvent::WebView(WebViewEvent::MessageRejected { source }) => {
                return Err(MetisError::protocol(
                    ErrorCode::InvalidOrigin,
                    format!("Message source is outside the packaged page: {source}"),
                ));
            }
            WebViewHostEvent::WebView(WebViewEvent::NewWindowDenied { .. }) => {
                if self.page.session.stage() == PageStage::Ready {
                    post(
                        surface,
                        &requests::error(
                            self.page.session.identity(),
                            None,
                            "new_window_denied",
                            ErrorCode::NavigationDenied,
                            "New-window navigation is disabled",
                        ),
                    )?;
                }
            }
            WebViewHostEvent::Window(_) | WebViewHostEvent::WebView(_) => {}
        }
        Ok(false)
    }

    fn permission_denied(
        &mut self,
        surface: &mut WebViewSurface,
        uri: &str,
        permission: WebViewPermission,
        user_initiated: bool,
    ) -> Result<()> {
        tracing::info!(event = "page_permission_denied", permission = %permission, user_initiated, source = %uri, "Actual native WebView2 permission denial");
        let identity = self.page.session.identity();
        if identity.role() == PageRole::PermissionProbe
            && matches!(
                self.page.session.stage(),
                PageStage::Loading | PageStage::Ready
            )
        {
            let kind = capability(permission)?;
            self.page
                .native_denial(uri, kind, user_initiated)
                .map_err(|error| protocol(&error))?;
            if self.page.session.stage() == PageStage::Ready {
                post(
                    surface,
                    &PageCommand::PermissionDenied {
                        identity,
                        kind,
                        user_initiated,
                    },
                )?;
            }
        } else {
            self.page
                .permission_source(uri)
                .map_err(|error| protocol(&error))?;
            if self.page.session.stage() == PageStage::Ready {
                post(
                    surface,
                    &requests::error(
                        identity,
                        None,
                        "permission_denied",
                        ErrorCode::PermissionDenied,
                        format!("WebView2 denied {permission} access request"),
                    ),
                )?;
            }
        }
        Ok(())
    }
}

fn capability(permission: WebViewPermission) -> Result<PageCapability> {
    match permission {
        WebViewPermission::Geolocation => Ok(PageCapability::Geolocation),
        WebViewPermission::Camera => Ok(PageCapability::Camera),
        WebViewPermission::Microphone => Ok(PageCapability::Microphone),
        WebViewPermission::Notifications => Ok(PageCapability::Notifications),
        _ => Err(MetisError::protocol(
            ErrorCode::PermissionDenied,
            format!("Permission {permission} is outside the four-kind probe"),
        )),
    }
}
