const VERTEX_SRC: &str = r#"
    #version 450 core
    layout (location = 0) in vec2 v_pos;
    layout (location = 1) in vec2 v_uv;
    layout (location = 2) in vec4 v_color;

    layout (location = 0) out vec2 a_uv;
    layout (location = 1) out vec4 a_color;

    layout (location = 0) uniform mat4 u_transform;

    void main() {
        a_uv = v_uv;
        a_color = v_color;
        gl_Position = u_transform * vec4(v_pos, 0.0, 1.0);
    }
"#;

const FRAGMENT_SRC: &str = r#"
    #version 450 core
    layout (location = 0) in vec2 a_uv;
    layout (location = 1) in vec4 a_color;

    layout (location = 0) out vec4 f_color;

    layout (binding = 0) uniform sampler2D u_texture;

    void main() {
       f_color = a_color * texture(u_texture, a_uv);
    }
"#;

pub struct Renderer<'grr> {
    device: &'grr grr::Device,
    pipeline: grr::Pipeline,
    textures: imgui::Textures<(grr::Image, grr::ImageView, grr::Sampler)>,
    vertex_array: grr::VertexArray,
}

impl<'grr> Renderer<'grr> {
    pub fn new(imgui: &mut imgui::ImGui, grr: &'grr grr::Device) -> Result<Self, grr::Error> {
        {
            // Fix incorrect colors with sRGB framebuffer
            fn imgui_gamma_to_linear(col: imgui::ImVec4) -> imgui::ImVec4 {
                let x = col.x.powf(2.2);
                let y = col.y.powf(2.2);
                let z = col.z.powf(2.2);
                let w = 1.0 - (1.0 - col.w).powf(2.2);
                imgui::ImVec4::new(x, y, z, w)
            }

            let style = imgui.style_mut();
            for col in 0..style.colors.len() {
                style.colors[col] = imgui_gamma_to_linear(style.colors[col]);
            }
        }

        let vs = grr.create_shader(grr::ShaderStage::Vertex, VERTEX_SRC.as_bytes())?;
        let fs = grr.create_shader(grr::ShaderStage::Fragment, FRAGMENT_SRC.as_bytes())?;

        let pipeline = grr.create_graphics_pipeline(grr::GraphicsPipelineDesc {
            vertex_shader: &vs,
            tessellation_control_shader: None,
            tessellation_evaluation_shader: None,
            geometry_shader: None,
            fragment_shader: Some(&fs),
        })?;

        let mut textures = imgui::Textures::new();
        let image = imgui.prepare_texture(|handle| {
            let image = grr
                .create_image(
                    grr::ImageType::D2 {
                        width: handle.width,
                        height: handle.height,
                        layers: 1,
                        samples: 1,
                    },
                    grr::Format::R8G8B8A8_SRGB,
                    1,
                )
                .unwrap();
            grr.object_name(&image, "imgui-texture");
            grr.copy_host_to_image(
                &image,
                grr::SubresourceLevel {
                    level: 0,
                    layers: 0..1,
                },
                grr::Offset { x: 0, y: 0, z: 0 },
                grr::Extent {
                    width: handle.width,
                    height: handle.height,
                    depth: 1,
                },
                &handle.pixels,
                grr::SubresourceLayout {
                    base_format: grr::BaseFormat::RGBA,
                    format_layout: grr::FormatLayout::U8,
                    row_pitch: handle.width,
                    image_height: handle.height,
                    alignment: 4,
                },
            );

            image
        });
        let image_view = grr.create_image_view(
            &image,
            grr::ImageViewType::D2,
            grr::Format::R8G8B8A8_SRGB,
            grr::SubresourceRange {
                layers: 0..1,
                levels: 0..1,
            },
        )?;
        let sampler = grr.create_sampler(grr::SamplerDesc {
            min_filter: grr::Filter::Linear,
            mag_filter: grr::Filter::Linear,
            mip_map: None,
            address: (
                grr::SamplerAddress::ClampEdge,
                grr::SamplerAddress::ClampEdge,
                grr::SamplerAddress::ClampEdge,
            ),
            lod_bias: 0.0,
            lod: 0.0..10.0,
            compare: None,
            border_color: [0.0, 0.0, 0.0, 1.0],
        })?;

        imgui.set_font_texture_id(textures.insert((image, image_view, sampler)));

        let vertex_array = grr.create_vertex_array(&[
            grr::VertexAttributeDesc {
                location: 0,
                binding: 0,
                format: grr::VertexFormat::Xy32Float,
                offset: 0,
            },
            grr::VertexAttributeDesc {
                location: 1,
                binding: 0,
                format: grr::VertexFormat::Xy32Float,
                offset: (2 * std::mem::size_of::<f32>()) as _,
            },
            grr::VertexAttributeDesc {
                location: 2,
                binding: 0,
                format: grr::VertexFormat::Xyzw8Unorm,
                offset: (4 * std::mem::size_of::<f32>()) as _,
            },
        ])?;

        Ok(Renderer {
            device: grr,
            pipeline,
            textures,
            vertex_array,
        })
    }

    pub fn render(&self, ui: imgui::Ui) -> Result<(), grr::Error> {
        let imgui::FrameSize {
            logical_size: (width, height),
            hidpi_factor,
        } = ui.frame_size();
        if width <= 0.0 || height <= 0.0 {
            return Ok(());
        }

        let fb_size = (
            (width * hidpi_factor) as f32,
            (height * hidpi_factor) as f32,
        );

        let transform = [
            [2.0 / width as f32, 0.0, 0.0, 0.0],
            [0.0, -2.0 / height as f32, 0.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
            [-1.0, 1.0, 0.0, 1.0],
        ];

        ui.render(|ui, mut draw_data| {
            draw_data.scale_clip_rects(ui.imgui().display_framebuffer_scale());
            for draw_list in &draw_data {
                self.render_draw_list(&draw_list, fb_size, &transform)?;
            }
            Ok(())
        })
    }

    fn render_draw_list<'a>(
        &self,
        draw_list: &imgui::DrawList<'a>,
        fb_size: (f32, f32),
        matrix: &[[f32; 4]; 4],
    ) -> Result<(), grr::Error> {
        let vertex_buffer = self.device.create_buffer_from_host(
            grr::as_u8_slice(&draw_list.vtx_buffer),
            grr::MemoryFlags::empty(),
        )?;
        let index_buffer = self.device.create_buffer_from_host(
            grr::as_u8_slice(&draw_list.idx_buffer),
            grr::MemoryFlags::empty(),
        )?;

        self.device.bind_pipeline(&self.pipeline);
        self.device.bind_vertex_array(&self.vertex_array);
        self.device
            .bind_index_buffer(&self.vertex_array, &index_buffer);
        self.device.bind_vertex_buffers(
            &self.vertex_array,
            0,
            &[grr::VertexBufferView {
                buffer: &vertex_buffer,
                offset: 0,
                stride: std::mem::size_of::<imgui::ImDrawVert>() as _,
                input_rate: grr::InputRate::Vertex,
            }],
        );

        let color_blend = grr::ColorBlend {
            attachments: vec![grr::ColorBlendAttachment {
                blend_enable: true,
                color: grr::BlendChannel {
                    src_factor: grr::BlendFactor::SrcAlpha,
                    dst_factor: grr::BlendFactor::OneMinusSrcAlpha,
                    blend_op: grr::BlendOp::Add,
                },
                alpha: grr::BlendChannel {
                    src_factor: grr::BlendFactor::One,
                    dst_factor: grr::BlendFactor::One,
                    blend_op: grr::BlendOp::Add,
                },
            }],
        };
        self.device.bind_color_blend_state(&color_blend);

        self.device
            .bind_uniform_constants(&self.pipeline, 0, &[grr::Constant::Mat4x4(*matrix)]);

        self.device.set_viewport(
            0,
            &[grr::Viewport {
                x: 0.0,
                y: 0.0,
                w: fb_size.0,
                h: fb_size.1,
                n: 0.0,
                f: 1.0,
            }],
        );

        let mut index_start = 0;
        for cmd in draw_list.cmd_buffer {
            let texture_id = cmd.texture_id.into();
            let (_, image_view, sampler) = self.textures.get(texture_id).unwrap(); // TODO

            self.device.bind_image_views(0, &[&image_view]);
            self.device.bind_samplers(0, &[&sampler]);

            self.device.set_scissor(
                0,
                &[grr::Region {
                    x: cmd.clip_rect.x as _,
                    y: (fb_size.1 - cmd.clip_rect.w) as _,
                    w: (cmd.clip_rect.z - cmd.clip_rect.x) as _,
                    h: (cmd.clip_rect.w - cmd.clip_rect.y) as _,
                }],
            );
            self.device.draw_indexed(
                grr::Primitive::Triangles,
                grr::IndexTy::U16,
                index_start..index_start + cmd.elem_count,
                0..1,
                0,
            );

            index_start += cmd.elem_count;
        }

        self.device.delete_buffer(vertex_buffer);
        self.device.delete_buffer(index_buffer);

        Ok(())
    }
}
