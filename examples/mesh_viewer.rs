//! Interactive software-rendered mesh viewer over the native Windows host.
//!
//! Gaia owns the geometry and the rasteriser; this example owns only the host
//! wiring: a native window, a mapping from host events to camera operations, and
//! a framebuffer the renderer writes into directly. Nothing here re-implements
//! projection, clipping, shading or depth.
//!
//! The viewer's input policy is expressed as semantic operations rather than as
//! host events, so the state and rendering half of this file builds and runs
//! everywhere while only the event translation is Windows-specific.
//!
//! Controls, with the model following the cursor:
//!
//! | Input | Effect |
//! | --- | --- |
//! | left drag | orbit about the target |
//! | middle or right drag | pan within the image plane |
//! | wheel | dolly in and out |
//! | arrow keys | orbit in fixed steps |
//! | `+` / `-` | dolly in and out in fixed steps |
//! | `F` | frame the whole mesh |
//! | `R` | reset the view to the initial framing |
//! | `C` | toggle back-face culling |
//! | `Esc` | quit |
//!
//! Run it with `cargo run --locked --example mesh_viewer`. Pass `--headless` to
//! render one frame offscreen, write `output/mesh-viewer/frame.bmp`, print the
//! render statistics and exit, which is the form a non-interactive gate can use.
#![cfg_attr(
    not(windows),
    expect(
        dead_code,
        reason = "the interactive bindings are driven only by the Windows host"
    )
)]

use gaia::IndexedMesh;
use gaia::application::render::{CullMode, OrbitCamera, RenderSettings, RenderStats, Renderer};
use gaia::domain::core::scalar::{Point3r, Real};
use gaia::domain::geometry::Aabb;
use metis_platform::Framebuffer;
use std::error::Error;

#[path = "support/framebuffer.rs"]
mod framebuffer_artifacts;

/// A boxed error that survives the host boundary, which needs `Send` and `Sync`
/// so a failure can cross the event loop.
type ViewerError = Box<dyn Error + Send + Sync>;

/// Default viewport width in pixels.
const FRAME_WIDTH: u32 = 960;
/// Default viewport height in pixels.
const FRAME_HEIGHT: u32 = 640;

/// Radians of orbit per pixel of drag.
const ORBIT_PER_PIXEL: f64 = 0.008;
/// Radians of orbit per arrow-key press.
const ORBIT_PER_KEY: f64 = 0.08;
/// Win32 `WHEEL_DELTA`: one scroll notch is this many wheel units.
const WHEEL_DELTA: f64 = 120.0;

/// Distance the camera starts at, before the first framing pass replaces it.
const INITIAL_DISTANCE: f64 = 4.0;
/// Starting yaw in radians, chosen so the first frame is a three-quarter view.
const INITIAL_YAW: f64 = 0.6;
/// Starting pitch in radians.
const INITIAL_PITCH: f64 = 0.35;

/// Which pointer drag is in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drag {
    /// Left button: orbit about the target.
    Orbit,
    /// Middle or right button: pan within the image plane.
    Pan,
}

/// A viewer action, named for its effect rather than for the key that produced
/// it, so the host owns the bindings and the viewer owns the meaning.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Shortcut {
    /// Frame the whole mesh.
    Frame,
    /// Return to the initial framing.
    Reset,
    /// Toggle back-face culling.
    ToggleCull,
    /// Orbit by a fixed step.
    OrbitBy {
        /// Radians of yaw.
        yaw: f64,
        /// Radians of pitch.
        pitch: f64,
    },
    /// Dolly by whole scroll notches.
    ZoomBy(f64),
}

/// Application state presented by the native host.
struct Viewer {
    frame: Framebuffer,
    renderer: Renderer,
    camera: OrbitCamera,
    mesh: IndexedMesh<Real>,
    settings: RenderSettings,
    drag: Option<Drag>,
    cursor: (i32, i32),
    frames: u64,
    last_stats: RenderStats,
}

impl Viewer {
    /// Build the viewer for a `width` x `height` viewport and draw once.
    ///
    /// # Errors
    /// Propagates a refused viewport size or a refused render.
    fn new(width: u32, height: u32) -> Result<Self, ViewerError> {
        let mut viewer = Self {
            frame: Framebuffer::new(width, height)?,
            renderer: Renderer::new(width, height)?,
            camera: initial_camera(),
            mesh: torus(64, 24),
            settings: RenderSettings::default(),
            drag: None,
            cursor: (0, 0),
            frames: 0,
            last_stats: RenderStats::default(),
        };
        viewer.frame_mesh();
        viewer.redraw()?;
        Ok(viewer)
    }

    /// The mesh's axis-aligned bounds, read from the pool itself so the viewer
    /// keeps no second copy of the extents.
    fn bounds(&self) -> Aabb<Real> {
        Aabb::from_points(self.mesh.vertices.positions())
    }

    /// Frame the whole mesh from the current angles.
    fn frame_mesh(&mut self) {
        let bounds = self.bounds();
        let aspect = f64::from(self.frame.width()) / f64::from(self.frame.height());
        self.camera.fit(&bounds, aspect);
    }

    /// Adopt a new viewport size, reallocating both buffers.
    ///
    /// # Errors
    /// Propagates a refused viewport size.
    fn resize(&mut self, width: u32, height: u32) -> Result<(), ViewerError> {
        self.frame = Framebuffer::new(width, height)?;
        self.renderer.resize(width, height)?;
        // The projection is aspect-driven, so the framing distance has to be
        // recomputed rather than reused.
        self.frame_mesh();
        Ok(())
    }

    /// Render the current view into the framebuffer, in place.
    ///
    /// # Errors
    /// Propagates a refused render.
    fn redraw(&mut self) -> Result<(), ViewerError> {
        // `pixels_mut` borrows the framebuffer while the renderer borrows
        // itself. The two fields are disjoint, so the renderer fills the host
        // buffer with no intermediate copy and no per-pixel call.
        let stats = self.renderer.render(
            &self.mesh,
            &self.camera,
            self.frame.pixels_mut(),
            &self.settings,
        )?;
        self.last_stats = stats;
        self.frames += 1;
        Ok(())
    }

    /// Begin a drag, recording where it started.
    fn drag_start(&mut self, drag: Drag, cursor: (i32, i32)) {
        self.drag = Some(drag);
        self.cursor = cursor;
    }

    /// End a drag, which a released button and a lost focus both do.
    fn drag_end(&mut self) {
        self.drag = None;
    }

    /// Apply a pointer delta in pixels, reporting whether the frame changed.
    fn drag_by(&mut self, dx: i32, dy: i32) -> bool {
        if dx == 0 && dy == 0 {
            return false;
        }
        let (dx, dy) = (f64::from(dx), f64::from(dy));
        match self.drag {
            // The model follows the cursor, so the eye moves opposite the
            // horizontal drag and with the vertical one.
            Some(Drag::Orbit) => self
                .camera
                .orbit(-dx * ORBIT_PER_PIXEL, dy * ORBIT_PER_PIXEL),
            Some(Drag::Pan) => self.camera.pan_pixels(dx, dy, self.frame.height()),
            None => return false,
        }
        true
    }

    /// Apply a scroll delta in Win32 wheel units.
    fn scroll(&mut self, wheel_units: f64) -> bool {
        if wheel_units == 0.0 || !wheel_units.is_finite() {
            return false;
        }
        // A wheel notch is positive upward, and positive steps zoom in.
        self.camera.zoom_steps(wheel_units / WHEEL_DELTA);
        true
    }

    /// Apply a shortcut, reporting whether the frame changed.
    fn shortcut(&mut self, shortcut: Shortcut) -> bool {
        match shortcut {
            Shortcut::Frame => self.frame_mesh(),
            Shortcut::Reset => {
                self.camera = initial_camera();
                self.frame_mesh();
            }
            Shortcut::ToggleCull => {
                self.settings.cull = match self.settings.cull {
                    CullMode::Back => CullMode::None,
                    CullMode::None => CullMode::Back,
                };
            }
            Shortcut::OrbitBy { yaw, pitch } => self.camera.orbit(yaw, pitch),
            Shortcut::ZoomBy(steps) => self.camera.zoom_steps(steps),
        }
        true
    }
}

/// The camera every session starts from.
fn initial_camera() -> OrbitCamera {
    let mut camera = OrbitCamera::new(Point3r::new(0.0, 0.0, 0.0), INITIAL_DISTANCE);
    camera.orbit(INITIAL_YAW, INITIAL_PITCH);
    camera
}

/// A closed, smooth, non-convex surface: it shows silhouette, occlusion and
/// shading without an asset file or a loader.
fn torus(major_segments: u32, minor_segments: u32) -> IndexedMesh<Real> {
    const MAJOR_RADIUS: f64 = 1.0;
    const MINOR_RADIUS: f64 = 0.4;

    let mut mesh = IndexedMesh::new();
    let mut rings = Vec::with_capacity(major_segments as usize);
    for major in 0..major_segments {
        let u = core::f64::consts::TAU * f64::from(major) / f64::from(major_segments);
        let (sin_u, cos_u) = u.sin_cos();
        let mut ring = Vec::with_capacity(minor_segments as usize);
        for minor in 0..minor_segments {
            let v = core::f64::consts::TAU * f64::from(minor) / f64::from(minor_segments);
            let (sin_v, cos_v) = v.sin_cos();
            let radius = MAJOR_RADIUS + MINOR_RADIUS * cos_v;
            ring.push(mesh.add_vertex_pos(Point3r::new(
                radius * cos_u,
                radius * sin_u,
                MINOR_RADIUS * sin_v,
            )));
        }
        rings.push(ring);
    }

    for major in 0..major_segments as usize {
        let next_major = (major + 1) % major_segments as usize;
        for minor in 0..minor_segments as usize {
            let next_minor = (minor + 1) % minor_segments as usize;
            let a = rings[major][minor];
            let b = rings[next_major][minor];
            let c = rings[next_major][next_minor];
            let d = rings[major][next_minor];
            // This winding points the face normal away from the tube axis, which
            // is what back-face culling and the headlight both assume.
            mesh.add_face(a, b, c);
            mesh.add_face(a, c, d);
        }
    }
    mesh
}

/// Render one frame offscreen and write it as a bitmap and a PNG artifact.
///
/// # Errors
/// Propagates a refused viewport, a refused render or a failed artifact write.
fn render_headless() -> Result<(), ViewerError> {
    let viewer = Viewer::new(FRAME_WIDTH, FRAME_HEIGHT)?;
    let output = std::path::Path::new("output/mesh-viewer");
    std::fs::create_dir_all(output)?;
    std::fs::write(
        output.join("frame.bmp"),
        framebuffer_artifacts::bmp_bytes(&viewer.frame)?,
    )?;
    std::fs::write(
        output.join("frame.png"),
        framebuffer_artifacts::png_bytes(&viewer.frame)?,
    )?;
    println!(
        "mesh-viewer: {}x{}, {} faces considered, {} triangles rasterized, \
         {} fragments, {} back-face culled, background {:#010X}",
        viewer.frame.width(),
        viewer.frame.height(),
        viewer.last_stats.faces_considered,
        viewer.last_stats.triangles_rasterized,
        viewer.last_stats.fragments_passed,
        viewer.last_stats.backface_culled,
        viewer.settings.background.packed(),
    );
    Ok(())
}

#[cfg(windows)]
mod host {
    use super::{Drag, FRAME_HEIGHT, FRAME_WIDTH, Shortcut, Viewer, ViewerError};
    use metis_platform::Framebuffer;
    use metis_platform::native::{
        MouseButton, NativeApplication, NativeFlow, WindowConfig, WindowEvent, WindowVisibility,
        run_native_application,
    };
    use std::io;
    use std::time::Duration;

    /// How long the host waits for a batch before handing an empty one over.
    const EVENT_WAIT: Duration = Duration::from_millis(100);

    /// Windows virtual-key code for `Esc`.
    const KEY_ESCAPE: u32 = 0x1B;
    /// Windows virtual-key code for `F`.
    const KEY_FRAME: u32 = 0x46;
    /// Windows virtual-key code for `R`.
    const KEY_RESET: u32 = 0x52;
    /// Windows virtual-key code for `C`.
    const KEY_CULL: u32 = 0x43;
    /// Windows virtual-key code for the left arrow.
    const KEY_LEFT: u32 = 0x25;
    /// Windows virtual-key code for the up arrow.
    const KEY_UP: u32 = 0x26;
    /// Windows virtual-key code for the right arrow.
    const KEY_RIGHT: u32 = 0x27;
    /// Windows virtual-key code for the down arrow.
    const KEY_DOWN: u32 = 0x28;
    /// Windows virtual-key codes for `+` on the main row and the keypad.
    const KEYS_ZOOM_IN: [u32; 2] = [0xBB, 0x6B];
    /// Windows virtual-key codes for `-` on the main row and the keypad.
    const KEYS_ZOOM_OUT: [u32; 2] = [0xBD, 0x6D];

    /// Translate a key press into a viewer action.
    ///
    /// `Esc` is handled before this is reached, so it has no action here.
    fn shortcut(virtual_key: u32) -> Option<Shortcut> {
        let orbit = super::ORBIT_PER_KEY;
        match virtual_key {
            KEY_FRAME => Some(Shortcut::Frame),
            KEY_RESET => Some(Shortcut::Reset),
            KEY_CULL => Some(Shortcut::ToggleCull),
            KEY_LEFT => Some(Shortcut::OrbitBy {
                yaw: -orbit,
                pitch: 0.0,
            }),
            KEY_RIGHT => Some(Shortcut::OrbitBy {
                yaw: orbit,
                pitch: 0.0,
            }),
            KEY_UP => Some(Shortcut::OrbitBy {
                yaw: 0.0,
                pitch: orbit,
            }),
            KEY_DOWN => Some(Shortcut::OrbitBy {
                yaw: 0.0,
                pitch: -orbit,
            }),
            key if KEYS_ZOOM_IN.contains(&key) => Some(Shortcut::ZoomBy(1.0)),
            key if KEYS_ZOOM_OUT.contains(&key) => Some(Shortcut::ZoomBy(-1.0)),
            _ => None,
        }
    }

    impl Viewer {
        /// Apply one host event, reporting whether the frame changed and whether
        /// the session is over.
        fn apply(&mut self, event: &WindowEvent) -> Result<(bool, bool), ViewerError> {
            match event {
                WindowEvent::CloseRequested | WindowEvent::Destroyed => Ok((false, true)),
                // A drag cannot continue while another window owns the pointer.
                WindowEvent::FocusLost => {
                    self.drag_end();
                    Ok((false, false))
                }
                WindowEvent::Resized { width, height } => {
                    if *width == 0
                        || *height == 0
                        || (*width == self.frame.width() && *height == self.frame.height())
                    {
                        return Ok((false, false));
                    }
                    self.resize(*width, *height)?;
                    Ok((true, false))
                }
                WindowEvent::PointerDown { x, y, button } => {
                    match button {
                        MouseButton::Left => self.drag_start(Drag::Orbit, (*x, *y)),
                        MouseButton::Middle | MouseButton::Right => {
                            self.drag_start(Drag::Pan, (*x, *y));
                        }
                        _ => {}
                    }
                    Ok((false, false))
                }
                WindowEvent::PointerUp { x, y, .. } => {
                    self.cursor = (*x, *y);
                    self.drag_end();
                    Ok((false, false))
                }
                WindowEvent::PointerMove { x, y } => {
                    let dx = x.saturating_sub(self.cursor.0);
                    let dy = y.saturating_sub(self.cursor.1);
                    self.cursor = (*x, *y);
                    Ok((self.drag_by(dx, dy), false))
                }
                WindowEvent::PointerWheel { delta_y, .. } => {
                    Ok((self.scroll(f64::from(*delta_y)), false))
                }
                // Auto-repeat is a legitimate held-key behaviour here, so it is
                // deliberately not filtered out.
                WindowEvent::KeyDown { virtual_key, .. } => {
                    if *virtual_key == KEY_ESCAPE {
                        return Ok((false, true));
                    }
                    if let Some(action) = shortcut(*virtual_key) {
                        Ok((self.shortcut(action), false))
                    } else {
                        Ok((false, false))
                    }
                }
                _ => Ok((false, false)),
            }
        }
    }

    impl NativeApplication for Viewer {
        type Error = io::Error;

        fn framebuffer(&self) -> &Framebuffer {
            &self.frame
        }

        fn handle_events(&mut self, events: &[WindowEvent]) -> Result<NativeFlow, Self::Error> {
            let mut repaint = false;
            for event in events {
                let (changed, quit) = self.apply(event).map_err(io::Error::other)?;
                repaint |= changed;
                if quit {
                    return Ok(NativeFlow::Exit);
                }
            }
            if repaint {
                self.redraw().map_err(io::Error::other)?;
            }
            // The host presents on request, so the viewer renders before asking
            // rather than inside the presentation callback.
            Ok(NativeFlow::Continue { repaint })
        }
    }

    /// Run the interactive session against a real window.
    pub(super) fn run() -> Result<(), ViewerError> {
        let config = WindowConfig::with_visibility(
            "Metis mesh viewer",
            FRAME_WIDTH,
            FRAME_HEIGHT,
            WindowVisibility::Visible,
        )?;
        run_native_application(&config, Viewer::new(FRAME_WIDTH, FRAME_HEIGHT)?, EVENT_WAIT)?;
        Ok(())
    }
}

fn main() -> Result<(), ViewerError> {
    if std::env::args_os().skip(1).any(|arg| arg == "--headless") {
        return render_headless();
    }
    run_interactive()
}

/// Run the interactive session on a host that can provide a window.
#[cfg(windows)]
fn run_interactive() -> Result<(), ViewerError> {
    host::run()
}

/// Report the missing host instead of pretending to run.
#[cfg(not(windows))]
fn run_interactive() -> Result<(), ViewerError> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the interactive mesh viewer needs the Windows native host; use --headless",
    )
    .into())
}
