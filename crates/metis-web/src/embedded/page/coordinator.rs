//! Exact admission and real bridge dispatch; callbacks borrow a weak coordinator.

use super::{application::Application, form, permissions, resources, view};
use crate::embedded::{
    PageCommand, PageEvent, PageIdentity, PageSession, PageStage, decode_page_command,
    encode_page_event,
};
use moirai_pal::wasm::{
    LocalTaskHandle, WebDocument, WebPermissions, WebTimer, WebViewBridge, WebViewMessageListener,
    spawn_local_with_handle,
};
use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
    time::Duration,
};

pub(super) struct Coordinator {
    pub(super) document: WebDocument,
    pub(super) bridge: WebViewBridge,
    pub(super) permissions: WebPermissions,
    pub(super) session: RefCell<PageSession>,
    pub(super) application: RefCell<Option<Application>>,
    pub(super) listener: RefCell<Option<WebViewMessageListener>>,
    pub(super) tasks: Rc<Cell<u32>>,
    pub(super) stop_task: RefCell<Option<LocalTaskHandle>>,
}

impl Coordinator {
    pub(super) fn start() -> io::Result<Rc<Self>> {
        let document = WebDocument::current()?;
        let initial = document
            .body()?
            .attribute("data-metis-mount")
            .ok_or_else(|| invalid("Packaged document has no mount command"))?;
        let PageCommand::Mount { identity } =
            decode_page_command(&initial, WebViewBridge::message_byte_limit())?
        else {
            return Err(invalid("Packaged document configuration is not a mount"));
        };
        let bridge = WebViewBridge::current()?;
        let page = Rc::new(Self {
            document,
            bridge,
            permissions: WebPermissions::current()?,
            session: RefCell::new(PageSession::new(identity)),
            application: RefCell::new(None),
            listener: RefCell::new(None),
            tasks: Rc::new(Cell::new(0)),
            stop_task: RefCell::new(None),
        });
        let weak = Rc::downgrade(&page);
        let listener = page.bridge.listen(move |message| {
            if let Some(page) = weak.upgrade() {
                let result = message
                    .and_then(|json| {
                        decode_page_command(&json, WebViewBridge::message_byte_limit())
                    })
                    .and_then(|command| page.dispatch(command));
                if let Err(error) = result {
                    page.failure(&error);
                }
            }
        })?;
        page.listener.replace(Some(listener));
        if let Err(error) = page.mount(identity) {
            page.failure(&error);
            return Err(error);
        }
        Ok(page)
    }

    pub(super) fn identity(&self) -> PageIdentity {
        self.session.borrow().identity()
    }

    pub(super) fn post(&self, event: &PageEvent) -> io::Result<()> {
        self.bridge.post_json(&encode_page_event(
            event,
            WebViewBridge::message_byte_limit(),
        )?)
    }

    pub(super) fn failure(&self, error: &io::Error) {
        let message = error.to_string();
        view::failure(&self.document, &message);
        if let Err(bridge_error) = self.post(&PageEvent::Failed {
            identity: self.identity(),
            message,
        }) {
            view::failure(
                &self.document,
                &format!("{error}; native failure publication: {bridge_error}"),
            );
        }
    }

    fn mount(self: &Rc<Self>, identity: PageIdentity) -> io::Result<()> {
        let stage = self.session.borrow().stage();
        if stage != PageStage::Loading {
            self.session.borrow_mut().remount(identity)?;
        } else if self.identity() != identity || self.application.borrow().is_some() {
            return Err(invalid("Mount does not match the admitted loading page"));
        }
        let application = Application::mount(self)?;
        self.application.replace(Some(application));
        self.session.borrow_mut().ready(identity)?;
        view::element(&self.document, "host-status")?
            .set_text("Host bridge connected; backend authority remains outside the page.");
        self.post(&PageEvent::Ready { identity })?;
        permissions::start_if_probe(self)
    }

    fn dispatch(self: &Rc<Self>, command: PageCommand) -> io::Result<()> {
        match command {
            PageCommand::Mount { identity } => self.mount(identity),
            PageCommand::Stop { identity } => self.stop(identity),
            reply @ (PageCommand::Result { .. } | PageCommand::Error { .. }) => {
                form::reply(self, reply)
            }
            PageCommand::PermissionDenied {
                identity,
                kind,
                user_initiated,
            } => {
                self.require_ready(identity)?;
                permissions::denied(self, kind, user_initiated)
            }
        }
    }

    pub(super) fn require_ready(&self, identity: PageIdentity) -> io::Result<()> {
        let session = self.session.borrow();
        if session.identity() != identity || session.stage() != PageStage::Ready {
            return Err(invalid(
                "Native command does not address the current ready page",
            ));
        }
        Ok(())
    }

    pub(super) fn stop(self: &Rc<Self>, identity: PageIdentity) -> io::Result<()> {
        self.session.borrow_mut().begin_stop(identity)?;
        if let Some(mut application) = self.application.borrow_mut().take() {
            application.close()?;
        }
        let weak = Rc::downgrade(self);
        // Cancellation wakes child tasks into the browser microtask queue. A
        // zero-delay timer yields to it; acknowledgement still checks ownership.
        let timer = WebTimer::new(Duration::ZERO)?;
        let handle = spawn_local_with_handle(async move {
            let result = timer.await;
            if let Some(page) = weak.upgrade() {
                let result = result
                    .and_then(|()| resources::snapshot(&page))
                    .and_then(|counts| {
                        page.session.borrow_mut().stopped(identity, counts)?;
                        page.post(&PageEvent::Stopped {
                            identity,
                            resources: counts,
                        })
                    });
                if let Err(error) = result {
                    page.failure(&error);
                }
            }
        });
        self.stop_task.replace(Some(handle));
        Ok(())
    }
}

pub(super) fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
