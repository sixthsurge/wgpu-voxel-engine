use std::sync::Arc;

use block::BlockId;
use chunk::CHUNK_SIZE_FLAT;
use fly_camera::FlyCamera;
use input::Input;
use render::{
    camera::Camera, camera::Projection, chunk_mesh_gen::ChunkMesh, context::RenderContext,
    engine::RenderEngine, mesh::Mesh,
};
use time::{TargetFrameRate, Time};
use util::transform::Transform;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    error::EventLoopError,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
use world::World;

mod block;
mod chunk;
mod fly_camera;
mod input;
mod render;
mod time;
mod util;
mod world;

const WINDOW_TITLE: &'static str = "\"minecraft\"";

/// Size of one degree in radians
const DEGREE: f32 = 180.0 / std::f32::consts::PI;

struct State {
    window: Arc<Window>,
    render_context: RenderContext,
    time: Time,
    input: Input,
    world: World,
    camera: Camera,
    render_engine: RenderEngine,
    fly_camera: FlyCamera,
    fly_camera_active: bool,
    close_requested: bool,
}

impl State {
    fn new(window: Arc<Window>) -> Self {
        let render_context = RenderContext::new(window.clone());
        let input = Input::new();
        let time = Time::new(TargetFrameRate::UnlimitedOrVsync);
        let camera = Camera::new(
            Transform::IDENTITY,
            Projection::Perspective {
                aspect_ratio: window.inner_size().width as f32 / window.inner_size().height as f32,
                fov_y_radians: 70.0 * DEGREE,
                z_near: 0.01,
                z_far: 1000.0,
            },
        );
        let world = World::new();
        let mut render_engine = RenderEngine::new(&render_context);

        let chunk_mesh = ChunkMesh::build([BlockId(0); CHUNK_SIZE_FLAT]);
        render_engine.add_chunk_mesh(Mesh::new(
            &render_context.device,
            &chunk_mesh.vertices,
            &chunk_mesh.indices,
        ));

        let fly_camera = FlyCamera::default();

        Self {
            window,
            render_context,
            time,
            input,
            camera,
            world,
            render_engine,
            fly_camera,
            fly_camera_active: true,
            close_requested: false,
        }
    }

    fn frame(&mut self) {
        self.time.begin_frame();
        self.update();
        self.render();
        self.time.update_frame_count();
        self.window.set_title(&format!(
            "{} ({} fps)",
            WINDOW_TITLE,
            self.time.get_frames_last_second()
        ));
        self.time.wait_for_next_frame();
    }

    fn resized(&mut self, new_size: PhysicalSize<u32>) {
        self.render_context.resized(new_size);
        self.camera.resized(new_size);
    }

    fn update(&mut self) {
        if self.fly_camera_active {
            self.fly_camera
                .update(&self.input, &self.time);
        }

        self.input.reset();
    }

    fn render(&mut self) {
        self.camera.transform = self.fly_camera.get_transform();
        self.render_engine
            .set_camera(&self.camera);

        let Some(surface_texture) = self
            .render_context
            .get_surface_texture()
        else {
            log::warn!("couldn't acquire surface texture");
            return;
        };

        let surface_texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.render_engine
            .render(&self.render_context, &surface_texture_view);

        surface_texture.present();
    }
}

struct WinitApplicationHandler {
    state: Option<State>,
}

impl WinitApplicationHandler {
    fn new() -> Self {
        Self { state: None }
    }
}

impl ApplicationHandler<()> for WinitApplicationHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let window_attributes = Window::default_attributes().with_title(WINDOW_TITLE);
            let window = Arc::new(
                event_loop
                    .create_window(window_attributes)
                    .expect("failed to create window"),
            );

            self.state = Some(State::new(window));
        }
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let state = self.state.as_mut().unwrap();

        match event {
            WindowEvent::CloseRequested => state.close_requested = true,
            WindowEvent::Resized(new_size) => state.resized(new_size),
            _ => {
                state.input.handle_window_event(&event);
            }
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        let state = self.state.as_mut().unwrap();
        state.input.handle_device_event(&event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.state.as_mut() {
            Some(state) => {
                if state.close_requested {
                    event_loop.exit();
                }
                state.frame();
                event_loop.set_control_flow(ControlFlow::Poll);
            }
            None => (),
        }
    }
}

fn main() -> Result<(), EventLoopError> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu=warn"))
        .init();
    EventLoop::new()?.run_app(&mut WinitApplicationHandler::new())
}