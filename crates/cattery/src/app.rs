//! A window showing one session.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use felis_client_core::{Offer, OutgoingFrame, ShadowScreen};
use felis_grid::image_layout::ImageLayout;
use felis_protocol::ImageId;
use felis_protocol::messages::{InputMsg, SessionInfo};
use felis_render_core::{ImageData, ImageSource};
use felis_render_wgpu::{DEFAULT_FONT_SIZE_LOGICAL_PX, Renderer, RendererConfig, SurfaceError};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::UnboundedSender;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

use crate::keys::{self, Stroke};
use crate::session::{self, Event};
use crate::{daemon, grid};

/// How long the daemon is given to end the session when its window closes.
const ENDING: Duration = Duration::from_secs(5);

/// Open a window on a new session of the daemon at `socket`, and run until
/// it closes.
pub fn run(socket: PathBuf) -> anyhow::Result<()> {
    let runtime = Runtime::new()?;
    let event_loop = EventLoop::<Event>::with_user_event().build()?;
    let (session, outgoing) = runtime.block_on({
        let proxy = event_loop.create_proxy();
        let socket = socket.clone();
        async move {
            let mut conn = daemon::connect(&socket, &std::env::current_exe()?).await?;
            let session = session::create(&mut conn, Vec::new(), &socket).await?;
            let outgoing = session::pump(conn, move |event| proxy.send_event(event).is_ok());
            anyhow::Ok((session, outgoing))
        }
    })?;
    let mut app = App {
        shadow: ShadowScreen::new(session.dims.rows, session.dims.cols),
        runtime,
        socket,
        session,
        outgoing: Some(outgoing),
        window: None,
        renderer: None,
        modifiers: ModifiersState::empty(),
        ended: false,
        failure: None,
    };
    event_loop.run_app(&mut app)?;
    app.failure.map_or(Ok(()), Err)
}

struct App {
    runtime: Runtime,
    socket: PathBuf,
    session: SessionInfo,
    /// Dropped when the window goes, which lets the writer drain and end.
    outgoing: Option<UnboundedSender<OutgoingFrame>>,
    shadow: ShadowScreen,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    modifiers: ModifiersState,
    /// Whether the program in the session has ended on its own.
    ended: bool,
    failure: Option<anyhow::Error>,
}

impl App {
    fn open(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let window =
            Arc::new(event_loop.create_window(Window::default_attributes().with_title("cattery"))?);
        let size = window.inner_size();
        let config = RendererConfig {
            font_size_physical_px: Some(font_size(&window)),
            ..RendererConfig::default()
        };
        let renderer = self.runtime.block_on(Renderer::new_with_config(
            Arc::clone(&window),
            size.width,
            size.height,
            config,
            None,
        ))?;
        self.renderer = Some(renderer);
        self.window = Some(window);
        self.fit(size);
        Ok(())
    }

    /// Resize the session to the cells the window now holds.
    fn fit(&mut self, size: PhysicalSize<u32>) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        renderer.resize(size.width, size.height);
        let cell = renderer.cell_metrics();
        let dims = grid::dims(size.width, size.height, cell.width, cell.height);
        self.shadow.local_resize(
            u16::try_from(dims.rows).unwrap_or(u16::MAX),
            u16::try_from(dims.cols).unwrap_or(u16::MAX),
        );
        self.send(&InputMsg::Resize { dims });
        self.redraw();
    }

    fn send(&self, msg: &InputMsg) {
        let Some(outgoing) = self.outgoing.as_ref() else {
            return;
        };
        match OutgoingFrame::input(msg) {
            // A closed queue means the connection ended, which the reader
            // reports on its own.
            Ok(frame) => _ = outgoing.send(frame),
            Err(err) => tracing::warn!(?err, ?msg, "input could not be encoded"),
        }
    }

    fn redraw(&self) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn draw(&mut self) {
        // A screen still being sent whole would flash half drawn.
        if self.shadow.is_rehydrating() {
            return;
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let drawn = renderer.render(
            self.shadow.screen(),
            &[],
            &NoImages,
            &ImageLayout::new(),
            self.shadow.viewport(),
            self.shadow.viewport_max(),
        );
        self.shadow.clear_damage();
        match drawn {
            Ok(()) => {}
            Err(SurfaceError::Lost | SurfaceError::Outdated) => renderer.reconfigure_surface(),
            Err(err) => tracing::warn!(?err, "the frame was not drawn"),
        }
        // Glyphs rasterized during this frame are drawn on the next one.
        if renderer.take_glyph_repaint_request() {
            self.redraw();
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, err: anyhow::Error) {
        self.failure.get_or_insert(err);
        event_loop.exit();
    }
}

impl ApplicationHandler<Event> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none()
            && let Err(err) = self.open(event_loop)
        {
            self.fail(event_loop, err);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.fit(size),
            WindowEvent::ScaleFactorChanged { .. } => {
                if let (Some(window), Some(renderer)) =
                    (self.window.clone(), self.renderer.as_mut())
                {
                    renderer.reload_font_size(font_size(&window));
                    self.fit(window.inner_size());
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                self.send(&keys::input(&Stroke::from(&event), self.modifiers));
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Event) {
        match event {
            Event::Grid(msg) => match self.shadow.apply(&msg) {
                Ok(()) => self.redraw(),
                Err(err) => self.fail(event_loop, err.into()),
            },
            Event::Exited => {
                self.ended = true;
                event_loop.exit();
            }
            Event::Closed => event_loop.exit(),
        }
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.outgoing = None;
        // Nothing reopens a session whose window closed, so one left
        // running would be out of reach until the daemon stops.
        if !self.ended {
            let ending = end(&self.socket, self.session.id);
            if let Err(err) = self
                .runtime
                .block_on(async { tokio::time::timeout(ENDING, ending).await? })
            {
                tracing::warn!(?err, id = self.session.id, "the session was left running");
            }
        }
    }
}

async fn end(socket: &Path, id: u128) -> anyhow::Result<()> {
    let mut ops = felis_client_core::connect(socket, Offer::ops()).await?;
    ops.destroy_session(format!("{id:032x}")).await?;
    Ok(())
}

fn font_size(window: &Window) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a display's scale factor is a small number"
    )]
    let scale = window.scale_factor() as f32;
    DEFAULT_FONT_SIZE_LOGICAL_PX * scale
}

/// Images are not shown yet.
struct NoImages;

impl ImageSource for NoImages {
    fn image_data(&self, _: ImageId) -> Option<ImageData<'_>> {
        None
    }
}
