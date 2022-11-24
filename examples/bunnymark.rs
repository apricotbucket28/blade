#![allow(irrefutable_let_patterns)]

use std::{ptr, time};

const BUNNY_SIZE: f32 = 0.15 * 256.0;
const GRAVITY: f32 = -9.8 * 100.0;
const MAX_VELOCITY: f32 = 750.0;

struct Globals {
    mvp_transform: [[f32; 4]; 4],
    sprite_size: [f32; 2],
    sprite_texture: lame::TextureView,
    sprite_sampler: lame::Sampler,
}

struct Locals {
    position: [f32; 2],
    velocity: [f32; 2],
    color: u32,
}

//TEMP
impl lame::ShaderData for Globals {
    fn layout() -> lame::ShaderDataLayout {
        lame::ShaderDataLayout {
            bindings: vec![
                (
                    "mvp_transform".to_string(),
                    lame::ShaderBinding::Plain {
                        ty: lame::PlainType::F32,
                        container: lame::PlainContainer::Matrix(
                            lame::VectorSize::Quad,
                            lame::VectorSize::Quad,
                        ),
                    },
                ),
                (
                    "sprite_size".to_string(),
                    lame::ShaderBinding::Plain {
                        ty: lame::PlainType::F32,
                        container: lame::PlainContainer::Vector(lame::VectorSize::Bi),
                    },
                ),
                (
                    "sprite_texture".to_string(),
                    lame::ShaderBinding::Texture {
                        dimension: lame::TextureViewDimension::D2,
                    },
                ),
                (
                    "sprite_sampler".to_string(),
                    lame::ShaderBinding::Sampler { comparison: false },
                ),
            ],
        }
    }
    fn fill<E: lame::ShaderDataEncoder>(&self, mut encoder: E) {
        encoder.set_plain(0, self.mvp_transform);
        encoder.set_plain(1, self.sprite_size);
        encoder.set_texture(2, self.sprite_texture);
        encoder.set_sampler(3, self.sprite_sampler);
    }
}

impl lame::ShaderData for Locals {
    fn layout() -> lame::ShaderDataLayout {
        lame::ShaderDataLayout {
            bindings: vec![
                (
                    "position".to_string(),
                    lame::ShaderBinding::Plain {
                        ty: lame::PlainType::F32,
                        container: lame::PlainContainer::Vector(lame::VectorSize::Bi),
                    },
                ),
                (
                    "velocity".to_string(),
                    lame::ShaderBinding::Plain {
                        ty: lame::PlainType::F32,
                        container: lame::PlainContainer::Vector(lame::VectorSize::Bi),
                    },
                ),
                (
                    "color".to_string(),
                    lame::ShaderBinding::Plain {
                        ty: lame::PlainType::U32,
                        container: lame::PlainContainer::Scalar,
                    },
                ),
            ],
        }
    }
    fn fill<E: lame::ShaderDataEncoder>(&self, mut encoder: E) {
        encoder.set_plain(0, self.position);
        encoder.set_plain(1, self.velocity);
        encoder.set_plain(2, self.color);
    }
}

struct Example {
    pipeline: lame::RenderPipeline,
    command_encoder: lame::CommandEncoder,
    prev_sync_point: Option<lame::SyncPoint>,
    _texture: lame::Texture,
    view: lame::TextureView,
    sampler: lame::Sampler,
    window_size: winit::dpi::PhysicalSize<u32>,
    bunnies: Vec<Locals>,
    rng: rand::rngs::ThreadRng,
    context: lame::Context,
}

impl Example {
    fn new(window: &winit::window::Window) -> Self {
        let window_size = window.inner_size();
        let context = unsafe {
            lame::Context::init_windowed(
                window,
                lame::ContextDesc {
                    validation: true,
                    capture: false,
                },
            )
            .unwrap()
        };

        let surface_format = context.resize(lame::SurfaceConfig {
            size: lame::Extent {
                width: window_size.width,
                height: window_size.height,
                depth: 1,
            },
            usage: lame::TextureUsage::TARGET,
            frame_count: 2,
        });

        let global_layout = <Globals as lame::ShaderData>::layout();
        let local_layout = <Locals as lame::ShaderData>::layout();
        let shader_source = std::fs::read_to_string("examples/bunnymark.wgsl").unwrap();
        let shader = context.create_shader(lame::ShaderDesc {
            source: &shader_source,
            data_layouts: &[Some(&global_layout), Some(&local_layout)],
        });

        let pipeline = context.create_render_pipeline(lame::RenderPipelineDesc {
            name: "main",
            vertex: shader.at("vs_main"),
            primitive: lame::PrimitiveState {
                topology: lame::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            fragment: shader.at("fs_main"),
            color_targets: &[lame::ColorTargetState {
                format: surface_format,
                blend: Some(lame::BlendState::ALPHA_BLENDING),
                write_mask: lame::ColorWrites::default(),
            }],
        });

        let extent = lame::Extent {
            width: 1,
            height: 1,
            depth: 1,
        };
        let texture = context.create_texture(lame::TextureDesc {
            name: "texutre",
            format: lame::TextureFormat::Rgba8Unorm,
            size: extent,
            dimension: lame::TextureDimension::D2,
            array_layers: 1,
            mip_level_count: 1,
            usage: lame::TextureUsage::RESOURCE | lame::TextureUsage::COPY,
        });
        let view = context.create_texture_view(lame::TextureViewDesc {
            name: "view",
            texture,
            format: lame::TextureFormat::Rgba8Unorm,
            dimension: lame::TextureViewDimension::D2,
            subresources: &Default::default(),
        });

        let upload_buffer = context.create_buffer(lame::BufferDesc {
            name: "staging",
            size: (extent.width * extent.height) as u64 * 4,
            memory: lame::Memory::Upload,
        });
        let texture_data = vec![0xFFu8; 4];
        unsafe {
            ptr::copy_nonoverlapping(
                texture_data.as_ptr(),
                upload_buffer.data(),
                texture_data.len(),
            );
        }

        let sampler = context.create_sampler(lame::SamplerDesc {
            name: "main",
            ..Default::default()
        });

        let mut bunnies = Vec::new();
        bunnies.push(Locals {
            position: [-100.0, 100.0],
            velocity: [10.0, 0.0],
            color: 0xFFFFFFFF,
        });

        let mut command_encoder =
            context.create_command_encoder(lame::CommandEncoderDesc { name: "main" });
        command_encoder.start();
        if let mut encoder = command_encoder.with_transfers() {
            encoder.copy_buffer_to_texture(upload_buffer.into(), 4, texture.into(), extent);
        }
        context.submit(&mut command_encoder);

        Self {
            pipeline,
            command_encoder,
            prev_sync_point: None,
            _texture: texture,
            view,
            sampler,
            window_size,
            bunnies,
            rng: rand::thread_rng(),
            context,
        }
    }

    fn increase(&mut self) {
        use rand::{Rng as _, RngCore as _};
        let spawn_count = 64 + self.bunnies.len() / 2;
        for _ in 0..spawn_count {
            let speed = self.rng.gen_range(-1.0..=1.0) * MAX_VELOCITY;
            self.bunnies.push(Locals {
                position: [0.0, 0.5 * (self.window_size.height as f32)],
                velocity: [speed, 0.0],
                color: self.rng.next_u32(),
            });
        }
        println!("Population: {} bunnies", self.bunnies.len());
    }

    fn step(&mut self, delta: f32) {
        for bunny in self.bunnies.iter_mut() {
            bunny.position[0] += bunny.velocity[0] * delta;
            bunny.position[1] += bunny.velocity[1] * delta;
            bunny.velocity[1] += GRAVITY * delta;
            if (bunny.velocity[0] > 0.0
                && bunny.position[0] + 0.5 * BUNNY_SIZE > self.window_size.width as f32)
                || (bunny.velocity[0] < 0.0 && bunny.position[0] - 0.5 * BUNNY_SIZE < 0.0)
            {
                bunny.velocity[0] *= -1.0;
            }
            if bunny.velocity[1] < 0.0 && bunny.position[1] < 0.5 * BUNNY_SIZE {
                bunny.velocity[1] *= -1.0;
            }
        }
    }

    fn render(&mut self) {
        let frame = self.context.acquire_frame();

        self.command_encoder.start();
        if let mut pass = self
            .command_encoder
            .with_render_targets(lame::RenderTargetSet {
                colors: &[lame::RenderTarget {
                    view: frame.texture_view(),
                    init_op: lame::InitOp::Clear(lame::TextureColor::TransparentBlack),
                    finish_op: lame::FinishOp::Store,
                }],
                depth_stencil: None,
            })
        {
            let mut rc = pass.with_pipeline(&self.pipeline);
            rc.bind_data(
                0,
                &Globals {
                    mvp_transform: [
                        [2.0 / self.window_size.width as f32, 0.0, 0.0, 0.0],
                        [0.0, 2.0 / self.window_size.height as f32, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [-1.0, -1.0, 0.0, 1.0],
                    ],
                    sprite_size: [BUNNY_SIZE; 2],
                    sprite_texture: self.view,
                    sprite_sampler: self.sampler,
                },
            );

            for local in self.bunnies.iter() {
                rc.bind_data(1, local);
                rc.draw(0, 4, 0, 1);
            }
        }

        let sync_point = self.context.submit(&mut self.command_encoder);
        if let Some(sp) = self.prev_sync_point.take() {
            self.context.wait_for(sp, !0);
        }
        self.context.present(frame);
        self.prev_sync_point = Some(sync_point);
    }

    fn deinit(&mut self) {
        //TODO
    }
}

fn main() {
    env_logger::init();

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("hal-bunnymark")
        .build(&event_loop)
        .unwrap();

    let mut example = Example::new(&window);
    let mut last_snapshot = time::Instant::now();
    let mut frame_count = 0;

    event_loop.run(move |event, _, control_flow| {
        let _ = &window; // force ownership by the closure
        *control_flow = winit::event_loop::ControlFlow::Poll;
        match event {
            winit::event::Event::RedrawEventsCleared => {
                window.request_redraw();
            }
            winit::event::Event::WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::KeyboardInput {
                    input:
                        winit::event::KeyboardInput {
                            virtual_keycode: Some(key_code),
                            state: winit::event::ElementState::Pressed,
                            ..
                        },
                    ..
                } => match key_code {
                    winit::event::VirtualKeyCode::Escape => {
                        *control_flow = winit::event_loop::ControlFlow::Exit;
                    }
                    winit::event::VirtualKeyCode::Space => {
                        example.increase();
                    }
                    _ => {}
                },
                winit::event::WindowEvent::CloseRequested => {
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                }
                _ => {}
            },
            winit::event::Event::RedrawRequested(_) => {
                frame_count += 1;
                if frame_count == 100 {
                    let accum_time = last_snapshot.elapsed().as_secs_f32();
                    println!(
                        "Avg frame time {}ms",
                        accum_time * 1000.0 / frame_count as f32
                    );
                    last_snapshot = time::Instant::now();
                    frame_count = 0;
                }
                example.step(0.01);
                example.render();
            }
            winit::event::Event::LoopDestroyed => {
                example.deinit();
            }
            _ => {}
        }
    })
}