#[macro_use]
extern crate imgui;

use glutin::dpi::LogicalSize;
use glutin::GlContext;

use imgui::{ImGui, ImGuiCond, ImFontConfig, FontGlyphRange};
use std::time::Instant;

fn main() -> grr::Result<()> {
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

    let mut imgui = ImGui::init();
    imgui.set_ini_filename(None);

    imgui_winit_support::configure_keys(&mut imgui);

    let hidpi_factor = window.get_hidpi_factor();
    let font_size = (13.0 * hidpi_factor) as f32;

    imgui.fonts().add_default_font_with_config(
        ImFontConfig::new()
            .oversample_h(1)
            .pixel_snap_h(true)
            .size_pixels(font_size),
    );

    imgui.set_font_global_scale((1.0 / hidpi_factor) as f32);

    let imgui_renderer = grr_imgui::Renderer::new(&mut imgui, &grr)?;

    let mut running = true;
    let mut last_frame = Instant::now();

    while running {
        events_loop.poll_events(|event| {
            imgui_winit_support::handle_event(
                &mut imgui,
                &event,
                window.get_hidpi_factor(),
                hidpi_factor,
            );
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
        }});

        let now = Instant::now();
        let delta = now - last_frame;
        let delta_s = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
        last_frame = now;

        imgui_winit_support::update_mouse_cursor(&imgui, &window);
        let frame_size = imgui_winit_support::get_frame_size(&window, hidpi_factor).unwrap();
        let ui = imgui.frame(frame_size, delta_s);
        ui.window(im_str!("Hello world"))
        .size((300.0, 100.0), ImGuiCond::FirstUseEver)
        .build(|| {
            ui.text(im_str!("Hello world!"));
            ui.text(im_str!("This...is...imgui-rs! with grr!"));
            ui.separator();
            let mouse_pos = ui.imgui().mouse_pos();
            ui.text(im_str!(
                "Mouse Position: ({:.1},{:.1})",
                mouse_pos.0,
                mouse_pos.1
            ));
        });

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

        imgui_renderer.render(ui);
        window.swap_buffers().unwrap();
    }

    Ok(())
}
