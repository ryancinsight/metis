use super::listeners::control_listeners;
use super::submission::submit;
use super::{
    BridgeStatus, BrowserApplication, BrowserState,
    config::{self, BridgeConfig},
    connect_failure_state, controls, generation_is_current, view,
};
use crate::epoch::Generation;
use metis_frontend::{AsyncFrontendApp, FormState};
use metis_ipc::{BrowserWebSocketTransport, client::HandshakeError};
use moirai_pal::wasm::{WebDocument, spawn_local_with_handle};
use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
    time::Duration,
};

impl BrowserApplication {
    pub(super) fn mount(document: &WebDocument, generation: Generation) -> io::Result<Self> {
        let bridge_config = config::read_bridge_config(document)?;
        let root = view::element(document, "metis-app")?;
        root.set_inner_html(controls::BROWSER_MARKUP);
        let state = Rc::new(RefCell::new(BrowserState::default()));
        view::render(document, &state.borrow())?;
        let app = Rc::new(RefCell::new(None));
        let task = Rc::new(RefCell::new(None));
        let drop_task = Rc::new(RefCell::new(None));
        let fragment_task = Rc::new(RefCell::new(None));
        let drop_sequence = Rc::new(Cell::new(0));

        let mut listeners = control_listeners(
            document,
            &state,
            &app,
            generation,
            &drop_task,
            &drop_sequence,
            &fragment_task,
        )?;

        let form = view::element(document, "metis-form")?;
        let listener_document = document.clone();
        let listener_state = Rc::clone(&state);
        let listener_app = Rc::clone(&app);
        let listener_task = Rc::clone(&task);
        let listener_fragment_task = Rc::clone(&fragment_task);
        listeners.push(form.add_event_listener("submit", move |event| {
            event.prevent_default();
            submit(
                &listener_document,
                &listener_state,
                &listener_app,
                &listener_task,
                &listener_fragment_task,
                generation,
            );
        })?);
        let application = Self {
            listeners,
            state,
            app,
            task,
            drop_task,
            fragment_task,
            generation,
        };
        view::render_lifecycle(
            document,
            application.listeners.len(),
            generation.value(),
            "Lifecycle: mounted; Rust-owned listeners active",
        )?;
        if let Some(config) = bridge_config {
            application.connect(document, config);
        }
        Ok(application)
    }

    fn connect(&self, document: &WebDocument, config: BridgeConfig) {
        let generation = self.generation;
        let state = Rc::clone(&self.state);
        let app_slot = Rc::clone(&self.app);
        let task_cleanup = Rc::clone(&self.task);
        let listener_document = document.clone();
        state.borrow_mut().bridge = BridgeStatus::Connecting;
        if let Err(error) = view::render(document, &state.borrow()) {
            view::set_mount_error(document, &error);
        }
        let task = spawn_local_with_handle(async move {
            let result = async {
                let transport = BrowserWebSocketTransport::connect_with_defaults_async(
                    &config.endpoint,
                    Duration::from_secs(5),
                )
                .await
                .map_err(HandshakeError::from)?;
                let mut frontend = AsyncFrontendApp::new(transport, Duration::from_secs(5))
                    .map_err(HandshakeError::from)?;
                let inputs = state.borrow().inputs.clone();
                frontend.set_inputs(
                    &inputs.patient_id,
                    inputs.weight_kg,
                    inputs.concentration_mg_ml,
                    inputs.target_dose_mcg_kg_min,
                );
                frontend.init(config.process_id, config.principal).await?;
                Ok::<_, HandshakeError>(frontend)
            }
            .await;
            if !generation_is_current(generation) {
                let _ = task_cleanup.borrow_mut().take();
                return;
            }
            match result {
                Ok(frontend) => {
                    let capabilities =
                        match (frontend.capabilities(), frontend.target_capabilities()) {
                            (Some(catalog), Some(target)) => {
                                view::capability_summary(catalog, target)
                            }
                            _ => "Host capabilities: unavailable".to_owned(),
                        };
                    *app_slot.borrow_mut() = Some(frontend);
                    let mut state = state.borrow_mut();
                    state.bridge = BridgeStatus::Ready;
                    state.state = FormState::Idle;
                    state.capabilities = capabilities;
                }
                Err(error) => {
                    let mut state = state.borrow_mut();
                    state.bridge = BridgeStatus::Disabled;
                    state.state = connect_failure_state(error);
                    "Host capabilities: unavailable".clone_into(&mut state.capabilities);
                }
            }
            if !generation_is_current(generation) {
                let _ = task_cleanup.borrow_mut().take();
                return;
            }
            if let Err(error) = view::render(&listener_document, &state.borrow()) {
                view::set_mount_error(&listener_document, &error);
            }
            let _ = task_cleanup.borrow_mut().take();
        });
        *self.task.borrow_mut() = Some(task);
    }
}

impl Drop for BrowserApplication {
    fn drop(&mut self) {
        let _ = self.task.borrow_mut().take();
        let _ = self.drop_task.borrow_mut().take();
        let _ = self.fragment_task.borrow_mut().take();
        self.listeners.clear();
    }
}
