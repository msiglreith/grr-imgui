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
    /// Create a new Rendered for an Imgui context.
    ///
    /// # Safety
    ///
    /// This function directly calls multiple unsafe `grr::Device`
    /// calls.
    pub unsafe fn new(
        imgui: &mut imgui::Context,
        grr: &'grr grr::Device,
    ) -> Result<Self, grr::Error> {
        {
            // Fix incorrect colors with sRGB framebuffer
            fn imgui_gamma_to_linear(col: [f32; 4]) -> [f32; 4] {
                let x = col[0].powf(2.2);
                let y = col[1].powf(2.2);
                let z = col[2].powf(2.2);
                let w = 1.0 - (1.0 - col[3]).powf(2.2);
                [x, y, z, w]
            }

            let style = imgui.style_mut();
            for col in 0..style.colors.len() {
                style.colors[col] = imgui_gamma_to_linear(style.colors[col]);
            }
        }

        let vs = grr.create_shader(
            grr::ShaderStage::Vertex,
            VERTEX_SRC.as_bytes(),
            grr::ShaderFlags::empty(),
        )?;
        let fs = grr.create_shader(
            grr::ShaderStage::Fragment,
            FRAGMENT_SRC.as_bytes(),
            grr::ShaderFlags::empty(),
        )?;

        let pipeline = grr.create_graphics_pipeline(
            grr::VertexPipelineDesc {
                vertex_shader: vs,
                tessellation_control_shader: None,
                tessellation_evaluation_shader: None,
                geometry_shader: None,
                fragment_shader: Some(fs),
            },
            grr::PipelineFlags::empty(),
        )?;

        let mut textures = imgui::Textures::new();
        let mut fonts = imgui.fonts();
        let image = {
            let texture = fonts.build_rgba32_texture();
            let image = grr
                .create_image(
                    grr::ImageType::D2 {
                        width: texture.width,
                        height: texture.height,
                        layers: 1,
                        samples: 1,
                    },
                    grr::Format::R8G8B8A8_SRGB,
                    1,
                )
                .unwrap();
            grr.object_name(image, "imgui-texture");
            grr.copy_host_to_image(
                &texture.data,
                image,
                grr::HostImageCopy {
                    host_layout: grr::MemoryLayout {
                        base_format: grr::BaseFormat::RGBA,
                        format_layout: grr::FormatLayout::U8,
                        row_length: texture.width,
                        image_height: texture.height,
                        alignment: 4,
                    },
                    image_subresource: grr::SubresourceLayers {
                        level: 0,
                        layers: 0..1,
                    },
                    image_offset: grr::Offset { x: 0, y: 0, z: 0 },
                    image_extent: grr::Extent {
                        width: texture.width,
                        height: texture.height,
                        depth: 1,
                    },
                },
            );

            image
        };
        let image_view = grr.create_image_view(
            image,
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

        fonts.tex_id = textures.insert((image, image_view, sampler));

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

    /// Render the current Imgui frame.
    ///
    /// # Safety
    ///
    /// This function makes multiple unsafe calls to the underlying
    /// `grr::Device`.
    pub unsafe fn render(&self, draw_data: &imgui::DrawData) -> Result<(), grr::Error> {
        let fb_width = draw_data.display_size[0] * draw_data.framebuffer_scale[0];
        let fb_height = draw_data.display_size[1] * draw_data.framebuffer_scale[1];

        if fb_width <= 0.0 || fb_height <= 0.0 {
            return Ok(());
        }

        let left = draw_data.display_pos[0];
        let right = draw_data.display_pos[0] + draw_data.display_size[0];
        let top = draw_data.display_pos[1];
        let bottom = draw_data.display_pos[1] + draw_data.display_size[1];

        let transform = [
            [2.0 / draw_data.display_size[0] as f32, 0.0, 0.0, 0.0],
            [0.0, -2.0 / draw_data.display_size[1] as f32, 0.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
            [
                -(right + left) / draw_data.display_size[0],
                (top + bottom) / draw_data.display_size[1],
                0.0,
                1.0,
            ],
        ];

        let clip_off = draw_data.display_pos;
        let clip_scale = draw_data.framebuffer_scale;

        for draw_list in draw_data.draw_lists() {
            self.render_draw_list(
                &draw_list,
                (fb_width, fb_height),
                &transform,
                clip_off,
                clip_scale,
            )?;
        }

        Ok(())
    }

    unsafe fn render_draw_list(
        &self,
        draw_list: &imgui::DrawList,
        fb_size: (f32, f32),
        matrix: &[[f32; 4]; 4],
        clip_off: [f32; 2],
        clip_scale: [f32; 2],
    ) -> Result<(), grr::Error> {
        let vertex_buffer = self.device.create_buffer_from_host(
            grr::as_u8_slice(&draw_list.vtx_buffer()),
            grr::MemoryFlags::empty(),
        )?;
        let index_buffer = self.device.create_buffer_from_host(
            grr::as_u8_slice(&draw_list.idx_buffer()),
            grr::MemoryFlags::empty(),
        )?;

        self.device.bind_pipeline(self.pipeline);
        self.device.bind_vertex_array(self.vertex_array);
        self.device
            .bind_index_buffer(self.vertex_array, index_buffer);
        self.device.bind_vertex_buffers(
            self.vertex_array,
            0,
            &[grr::VertexBufferView {
                buffer: vertex_buffer,
                offset: 0,
                stride: std::mem::size_of::<imgui::DrawVert>() as _,
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
            .bind_uniform_constants(self.pipeline, 0, &[grr::Constant::Mat4x4(*matrix)]);

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
        for cmd in draw_list.commands() {
            match cmd {
                imgui::DrawCmd::Elements {
                    count,
                    cmd_params:
                        imgui::DrawCmdParams {
                            clip_rect,
                            texture_id,
                            ..
                        },
                } => {
                    let clip_rect = [
                        (clip_rect[0] - clip_off[0]) * clip_scale[0],
                        (clip_rect[1] - clip_off[1]) * clip_scale[1],
                        (clip_rect[2] - clip_off[0]) * clip_scale[0],
                        (clip_rect[3] - clip_off[1]) * clip_scale[1],
                    ];

                    if clip_rect[0] < fb_size.0
                        && clip_rect[1] < fb_size.1
                        && clip_rect[2] >= 0.0
                        && clip_rect[3] >= 0.0
                    {
                        let (_, image_view, sampler) = self.textures.get(texture_id).unwrap(); // TODO
                        self.device.bind_image_views(0, &[*image_view]);
                        self.device.bind_samplers(0, &[*sampler]);

                        self.device.set_scissor(
                            0,
                            &[grr::Region {
                                x: clip_rect[0] as _,
                                y: (fb_size.1 - clip_rect[3]) as _,
                                w: (clip_rect[2] - clip_rect[0]).abs().ceil() as _,
                                h: (clip_rect[3] - clip_rect[1]).abs().ceil() as _,
                            }],
                        );

                        self.device.draw_indexed(
                            grr::Primitive::Triangles,
                            grr::IndexTy::U16,
                            index_start..index_start + count as u32,
                            0..1,
                            0,
                        );
                    }
                    index_start += count as u32;
                }

                _ => unimplemented!(),
            }
        }

        self.device.delete_buffer(vertex_buffer);
        self.device.delete_buffer(index_buffer);

        Ok(())
    }
}
