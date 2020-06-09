use glutin::dpi::{LogicalSize, PhysicalSize};
use glutin::platform::desktop::EventLoopExtDesktop;

use std::time::Instant;

fn main() -> grr::Result<()> {
    let mut event_loop = glutin::event_loop::EventLoop::new();
    let wb = glutin::window::WindowBuilder::new()
        .with_title("Hello, world!")
        .with_inner_size(LogicalSize {
            width: 1024.0,
            height: 768.0,
        });

    let window = unsafe {
        glutin::ContextBuilder::new()
            .with_vsync(true)
            .with_srgb(true)
            .with_gl_debug_flag(true)
            .build_windowed(wb, &event_loop)
            .unwrap()
            .make_current()
            .unwrap()
    };

    let PhysicalSize {
        width: mut w,
        height: mut h,
    } = window.window().inner_size();

    let grr = unsafe {
        grr::Device::new(
            |symbol| window.get_proc_address(symbol) as *const _,
            grr::Debug::Enable {
                callback: |_, _, _, _, msg| {
                    println!("{:?}", msg);
                },
                flags: grr::DebugReport::FULL,
            },
        )
    };

    let mut imgui = imgui::Context::create();
    imgui.set_ini_filename(None);

    let mut platform = imgui_winit_support::WinitPlatform::init(&mut imgui);

    let hidpi_factor = window.window().scale_factor();
    let font_size = (13.0 * hidpi_factor) as f32;

    imgui
        .fonts()
        .add_font(&[imgui::FontSource::DefaultFontData {
            config: Some(imgui::FontConfig {
                size_pixels: font_size,
                ..imgui::FontConfig::default()
            }),
        }]);

    imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

    let imgui_renderer = unsafe { grr_imgui::Renderer::new(&mut imgui, &grr)? };

    platform.attach_window(
        imgui.io_mut(),
        window.window(),
        imgui_winit_support::HiDpiMode::Rounded,
    );

    let mut running = true;
    let mut last_frame = Instant::now();

    event_loop.run_return(|event, _, control_flow| {
        unsafe {
            platform.handle_event(imgui.io_mut(), window.window(), &event);
            *control_flow = glutin::event_loop::ControlFlow::Poll;
            match event {
                glutin::event::Event::MainEventsCleared => {}
                glutin::event::Event::WindowEvent { event, .. } => match event {
                    glutin::event::WindowEvent::CloseRequested => {
                        *control_flow = glutin::event_loop::ControlFlow::Exit;
                        return;
                    }
                    glutin::event::WindowEvent::Resized(size) => {
                        w = size.width;
                        h = size.height;
                        //window.resize(size);
                        return;
                    }
                    _ => {
                        return;
                    }
                },
                _ => {
                    return;
                }
            }

            let io = imgui.io_mut();
            platform
                .prepare_frame(io, window.window())
                .expect("Failed to start frame");
            last_frame = io.update_delta_time(last_frame);
            let ui = imgui.frame();
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

            imgui_renderer.render(ui.render()).unwrap();
            window.swap_buffers().unwrap();
        }
    });

    Ok(())
}
