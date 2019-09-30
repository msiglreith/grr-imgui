#[macro_use]
extern crate imgui;

use glutin::dpi::LogicalSize;
use glutin::GlContext;

use std::time::Instant;

fn main() -> grr::Result<()> {
    unsafe {
        let mut events_loop = glutin::EventsLoop::new();
        let window = glutin::WindowBuilder::new()
            .with_title("Hello, world!")
            .with_dimensions(LogicalSize {
                width: 1024.0,
                height: 768.0,
            });
        let context = glutin::ContextBuilder::new()
            .with_vsync(true)
            .with_srgb(true)
            .with_gl_debug_flag(true);

        let window = glutin::GlWindow::new(window, context, &events_loop).unwrap();

        let LogicalSize {
            width: w,
            height: h,
        } = window.get_inner_size().unwrap();

        unsafe {
            window.make_current().unwrap();
        }

        let grr = grr::Device::new(
            |symbol| window.get_proc_address(symbol) as *const _,
            grr::Debug::Enable {
                callback: |_, _, _, _, msg| {
                    println!("{:?}", msg);
                },
                flags: grr::DebugReport::FULL,
            },
        );

        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(None);

        let mut platform = imgui_winit_support::WinitPlatform::init(&mut imgui);

        let hidpi_factor = window.get_hidpi_factor();
        let font_size = (13.0 * hidpi_factor) as f32;

        imgui
            .fonts()
            .add_font(&[imgui::FontSource::DefaultFontData {
                config: Some(imgui::FontConfig {
                    size_pixels: font_size,
                    ..imgui::FontConfig::default()
                }),
            }]);

        imgui.io_mut().font_global_scale = ((1.0 / hidpi_factor) as f32);

        let imgui_renderer = grr_imgui::Renderer::new(&mut imgui, &grr)?;
        platform.attach_window(
            imgui.io_mut(),
            window.window(),
            imgui_winit_support::HiDpiMode::Rounded,
        );

        let mut running = true;
        let mut last_frame = Instant::now();

        while running {
            events_loop.poll_events(|event| {
                platform.handle_event(imgui.io_mut(), window.window(), &event);
                match event {
                    glutin::Event::WindowEvent { event, .. } => match event {
                        glutin::WindowEvent::CloseRequested => running = false,
                        glutin::WindowEvent::Resized(size) => {
                            let dpi_factor = window.get_hidpi_factor();
                            window.resize(size.to_physical(dpi_factor));
                        }
                        _ => (),
                    },
                    _ => (),
                }
            });

            let io = imgui.io_mut();
            platform
                .prepare_frame(io, window.window())
                .expect("Failed to start frame");
            last_frame = io.update_delta_time(last_frame);
            let mut ui = imgui.frame();
            ui.show_demo_window(&mut running);

            grr.set_viewport(
                0,
                &[grr::Viewport {
                    x: 0.0,
                    y: 0.0,
                    w: w as _,
                    h: h as _,
                    n: 0.0,
                    f: 1.0,
                }],
            );
            grr.set_scissor(
                0,
                &[grr::Region {
                    x: 0,
                    y: 0,
                    w: w as _,
                    h: h as _,
                }],
            );

            grr.clear_attachment(
                grr::Framebuffer::DEFAULT,
                grr::ClearAttachment::ColorFloat(0, [0.5, 0.5, 0.5, 1.0]),
            );

            imgui_renderer.render(ui.render());
            window.swap_buffers().unwrap();
        }
    }

    Ok(())
}
