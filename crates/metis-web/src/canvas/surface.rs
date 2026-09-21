//! Moirai-backed browser canvas surface.

use super::{CanvasEvent, CanvasEventError, CanvasFrame};
use moirai_pal::wasm::{CanvasSize, RgbaFrame, WebCanvas, WebDocument, WebGpuCanvas};
use std::io;

mod input;

use input::CanvasInput;

/// A browser canvas surface owned by the Metis host.
pub struct CanvasSurface {
    canvas: CanvasRenderer,
    input: Option<CanvasInput>,
}

enum CanvasRenderer {
    Raster(WebCanvas),
    WebGpu(WebGpuCanvas),
}

impl CanvasRenderer {
    fn id(&self) -> String {
        match self {
            Self::Raster(canvas) => canvas.id(),
            Self::WebGpu(canvas) => canvas.id(),
        }
    }

    fn present(&self, frame: RgbaFrame<'_>) -> io::Result<()> {
        match self {
            Self::Raster(canvas) => canvas.present(frame),
            Self::WebGpu(canvas) => canvas.present(frame),
        }
    }

    async fn recreate(&mut self) -> io::Result<()> {
        match self {
            Self::Raster(_) => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "WebGPU recovery requires an explicit WebGPU surface",
            )),
            Self::WebGpu(canvas) => canvas.recreate().await,
        }
    }
}

impl CanvasSurface {
    /// Returns the number of DOM listener guards owned by this surface.
    ///
    /// A surface created without input owns no listeners. This counts retained
    /// guards, not browser-private memory or listeners installed by other code.
    #[must_use]
    pub fn listener_count(&self) -> usize {
        self.input.as_ref().map_or(0, CanvasInput::listener_count)
    }

    /// Resolves a canvas from the current browser document by identifier.
    ///
    /// # Errors
    /// Returns a typed I/O error when no browser document exists, the element
    /// is absent, is not a canvas, or cannot provide a two-dimensional context.
    pub fn from_current_document(id: &str) -> io::Result<Self> {
        let document = WebDocument::current()?;
        Self::from_document(&document, id)
    }

    /// Resolves a canvas from a document by its stable identifier.
    ///
    /// # Errors
    /// Returns a typed I/O error when the element is absent, is not a canvas,
    /// or cannot provide a two-dimensional rendering context.
    pub fn from_document(document: &WebDocument, id: &str) -> io::Result<Self> {
        Ok(Self {
            canvas: CanvasRenderer::Raster(document.canvas_by_id(id)?),
            input: None,
        })
    }

    /// Resolves a canvas and asynchronously acquires its WebGPU device.
    ///
    /// WebGPU is selected explicitly. The constructor returns an unsupported
    /// error when the browser cannot provide an adapter; it never falls back
    /// to the two-dimensional provider.
    ///
    /// # Errors
    /// Returns the provider's typed error when the canvas is absent, is not a
    /// canvas, or WebGPU adapter/device setup fails.
    pub async fn from_current_document_gpu(id: &str) -> io::Result<Self> {
        let document = WebDocument::current()?;
        Self::from_document_gpu(&document, id).await
    }

    /// Resolves a canvas from a document and asynchronously acquires WebGPU.
    ///
    /// # Errors
    /// Returns the provider's typed error when the canvas is absent, is not a
    /// canvas, or WebGPU adapter/device setup fails.
    pub async fn from_document_gpu(document: &WebDocument, id: &str) -> io::Result<Self> {
        Ok(Self {
            canvas: CanvasRenderer::WebGpu(document.gpu_canvas_by_id(id).await?),
            input: None,
        })
    }

    /// Resolves a canvas and retains bounded pointer, wheel and keyboard listeners.
    ///
    /// The listeners are removed when this surface is dropped. Events remain
    /// format-neutral; the consuming application decides how coordinates,
    /// buttons and deltas affect its state.
    ///
    /// # Errors
    /// Returns a typed I/O error when the canvas cannot be resolved or the
    /// browser rejects one of the listener registrations.
    pub fn from_current_document_with_input(id: &str) -> io::Result<Self> {
        let document = WebDocument::current()?;
        Self::from_document_with_input(&document, id)
    }

    /// Resolves a canvas and retains bounded pointer, wheel and keyboard listeners.
    ///
    /// # Errors
    /// Returns a typed I/O error when the canvas cannot be resolved or the
    /// browser rejects one of the listener registrations.
    pub fn from_document_with_input(document: &WebDocument, id: &str) -> io::Result<Self> {
        let element = document.get_element_by_id(id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "canvas element identifier is absent",
            )
        })?;
        let canvas = WebCanvas::from_element(&element)?;
        let input = CanvasInput::attach(element)?;
        Ok(Self {
            canvas: CanvasRenderer::Raster(canvas),
            input: Some(input),
        })
    }

    /// Resolves a canvas, acquires WebGPU and retains bounded input listeners.
    ///
    /// WebGPU is selected explicitly. Pointer, wheel and keyboard events stay
    /// format-neutral, and listener guards are removed when the surface drops.
    ///
    /// # Errors
    /// Returns the provider's typed error when the canvas or WebGPU device
    /// cannot be resolved, or when listener registration fails.
    pub async fn from_current_document_gpu_with_input(id: &str) -> io::Result<Self> {
        let document = WebDocument::current()?;
        Self::from_document_gpu_with_input(&document, id).await
    }

    /// Resolves a canvas, acquires WebGPU and retains bounded input listeners.
    ///
    /// # Errors
    /// Returns the provider's typed error when the canvas or WebGPU device
    /// cannot be resolved, or when listener registration fails.
    pub async fn from_document_gpu_with_input(
        document: &WebDocument,
        id: &str,
    ) -> io::Result<Self> {
        let element = document.get_element_by_id(id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "canvas element identifier is absent",
            )
        })?;
        let canvas = WebGpuCanvas::from_element(&element).await?;
        let input = CanvasInput::attach(element)?;
        Ok(Self {
            canvas: CanvasRenderer::WebGpu(canvas),
            input: Some(input),
        })
    }

    /// Returns the resolved canvas identifier.
    #[must_use]
    pub fn id(&self) -> String {
        self.canvas.id()
    }

    /// Recreates the explicit WebGPU provider after device or swap-chain loss.
    ///
    /// The surface keeps its canvas element and any retained input listeners;
    /// only the provider's browser GPU handles are replaced. Raster surfaces
    /// return [`io::ErrorKind::Unsupported`] because recovery must not silently
    /// change the requested presentation backend.
    ///
    /// # Errors
    /// Returns the provider's setup error when a fresh adapter or device cannot
    /// be acquired, or [`io::ErrorKind::Unsupported`] for a raster surface.
    pub async fn recreate(&mut self) -> io::Result<()> {
        self.canvas.recreate().await
    }

    /// Presents one borrowed RGBA8 frame without retaining its bytes.
    ///
    /// Dimensions and exact storage length are validated by Moirai's shared
    /// browser frame contract before the Web API upload. The consumer remains
    /// the owner of the source bytes and any domain meaning attached to them.
    ///
    /// # Errors
    /// Returns [`io::ErrorKind::InvalidInput`] for zero, oversized, or
    /// inconsistent dimensions/storage, or when the browser rejects the
    /// upload.
    pub fn present<F>(&self, frame: &F) -> io::Result<()>
    where
        F: CanvasFrame + ?Sized,
    {
        let size = CanvasSize::new(frame.width(), frame.height())?;
        if let Some(spacing) = frame.display_spacing() {
            spacing.aspect(frame.width(), frame.height())?;
        }
        let frame = RgbaFrame::new(size, frame.rgba())?;
        self.canvas.present(frame)
    }

    /// Takes all events captured since the previous call.
    ///
    /// The returned batch is bounded by [`super::CANVAS_EVENT_CAPACITY`]. A
    /// queue overflow or browser metadata/capture failure clears the pending
    /// batch and returns a typed error so the consumer can cancel its gesture
    /// and decide whether to remount the surface.
    ///
    /// # Errors
    /// Returns [`CanvasEventError`] when browser metadata is invalid, pointer
    /// capture fails, or the bounded event queue overflowed.
    pub fn take_events(&self) -> Result<Box<[CanvasEvent]>, CanvasEventError> {
        self.input.as_ref().map_or_else(
            || Ok(Vec::new().into_boxed_slice()),
            CanvasInput::take_events,
        )
    }
}
