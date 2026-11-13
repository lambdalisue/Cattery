//! A window of Cattery's own drawn with egui, as Preferences and Sessions
//! are: the winit window, egui's state for it, and what paints it.

use std::num::NonZeroU32;
use std::sync::Arc;

use egui::{Ui, ViewportId};
use egui_wgpu::winit::Painter;
use egui_wgpu::{RendererOptions, WgpuConfiguration, WgpuSetup};
use tokio::runtime::Runtime;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

use crate::look;

pub struct Surface {
    window: Arc<Window>,
    context: egui::Context,
    input: egui_winit::State,
    painter: Painter,
}

impl Surface {
    /// Open a window of `attributes`, drawing as Cattery's own windows do.
    pub fn open(
        event_loop: &ActiveEventLoop,
        runtime: &Runtime,
        attributes: WindowAttributes,
    ) -> anyhow::Result<Self> {
        let window = Arc::new(event_loop.create_window(attributes)?);
        let context = egui::Context::default();
        look::install(&context);
        // felis's renderer keeps its device to itself, so egui has one of
        // its own.
        let config = WgpuConfiguration {
            wgpu_setup: WgpuSetup::from_display_handle(event_loop.owned_display_handle()),
            ..WgpuConfiguration::default()
        };
        let mut painter = runtime.block_on(Painter::new(
            context.clone(),
            config,
            false,
            RendererOptions::default(),
        ));
        runtime.block_on(painter.set_window(ViewportId::ROOT, Some(Arc::clone(&window))))?;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a display's scale factor is a small number"
        )]
        let scale = window.scale_factor() as f32;
        let input = egui_winit::State::new(
            context.clone(),
            ViewportId::ROOT,
            &window,
            Some(scale),
            window.theme(),
            painter.max_texture_side(),
        );
        Ok(Self {
            window,
            context,
            input,
            painter,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn context(&self) -> &egui::Context {
        &self.context
    }

    pub fn focus(&self) {
        self.window.focus_window();
    }

    pub fn set_icon(&self, icon: Option<winit::window::Icon>) {
        self.window.set_window_icon(icon);
    }

    pub fn redraw(&self) {
        self.window.request_redraw();
    }

    /// Take `event`; whether it is time to draw a frame.
    pub fn take(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::RedrawRequested => return true,
            WindowEvent::Resized(size) => {
                if let (Some(width), Some(height)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                {
                    self.painter
                        .on_window_resized(ViewportId::ROOT, width, height);
                }
            }
            _ => {}
        }
        if self.input.on_window_event(&self.window, event).repaint {
            self.window.request_redraw();
        }
        false
    }

    /// Draw a frame of what `ui` lays out.
    pub fn frame(&mut self, ui: impl FnMut(&mut Ui)) {
        let raw = self.input.take_egui_input(&self.window);
        let mut output = self.context.run_ui(raw, ui);
        self.input
            .handle_platform_output(&self.window, output.platform_output);
        let shapes = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        self.painter.paint_and_update_textures(
            ViewportId::ROOT,
            output.pixels_per_point,
            [0.0, 0.0, 0.0, 1.0],
            &shapes,
            &mut output.textures_delta,
            Vec::new(),
            &self.window,
        );
        if output
            .viewport_output
            .get(&ViewportId::ROOT)
            .is_some_and(|viewport| viewport.repaint_delay.is_zero())
        {
            self.window.request_redraw();
        }
    }
}
