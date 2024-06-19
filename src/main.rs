use std::sync::Arc;

use block::{BLOCK_AIR, BLOCK_DIRT, BLOCK_GRASS, BLOCK_LAMP_ORANGE};
use fly_camera::FlyCamera;
use generational_arena::Index;
use input::Input;
use render::{render_context::RenderContext, render_engine::RenderEngine};
use tasks::Tasks;
use terrain::{chunk::CHUNK_SIZE, load_area::LoadArea, position_types::ChunkPos, Terrain};
use time::{TargetFrameRate, Time};
use util::size::Size3;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, PhysicalSize},
    error::EventLoopError,
    event::{DeviceEvent, DeviceId, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::KeyCode,
    window::{Window, WindowId},
};

use crate::{block::BLOCK_WOOD, terrain::position_types::GlobalBlockPos};

mod block;
mod fly_camera;
mod input;
mod render;
mod tasks;
mod terrain;
mod time;
mod util;

const WINDOW_TITLE: &'static str = "\"minecraft\"";

/// Number of threads to use for task processing
const TASKS_WORKER_THREAD_COUNT: usize = 4;

/// Priority value for chunk mesh generation tasks when an outdated mesh already exists
const CHUNK_MESH_UPDATE_PRIORITY: i32 = 0;

/// Priority value for chunk mesh generation tasks when no mesh already exists
const CHUNK_MESH_GENERATION_PRIORITY: i32 = 1;

/// Priority value for chunk loading tasks
const CHUNK_LOADING_PRIORITY: i32 = 2;

/// Priority value for chunk mesh generation tasks when an up-to-date mesh already exists
const CHUNK_MESH_OPTIMIZATION_PRIORITY: i32 = 3;

struct State {
    window: Arc<Window>,
    render_context: RenderContext,
    time: Time,
    input: Input,
    tasks: Tasks,
    terrain: Terrain,
    load_area_index: Index,
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
        let tasks = Tasks::new(TASKS_WORKER_THREAD_COUNT);
        let mut terrain = Terrain::new();
        let fly_camera = FlyCamera::default();

        let load_area_index = terrain
            .load_areas_mut()
            .insert(LoadArea::new(
                ChunkPos::ZERO,
                Size3::new(40, 16, 40),
                terrain::load_area::AreaShape::Cylindrical,
            ));
        let render_engine = RenderEngine::new(
            &render_context,
            terrain
                .load_areas()
                .get(load_area_index)
                .unwrap(),
        );

        Self {
            window,
            render_context,
            input,
            time,
            tasks,
            terrain,
            load_area_index,
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
        self.time.wait_for_next_frame();
    }

    fn resized(&mut self, new_size: PhysicalSize<u32>) {
        self.render_context.resized(new_size);
        self.render_engine
            .resized(&self.render_context);
    }

    fn update(&mut self) {
        self.terrain.clear_events();

        // capture cursor
        if self.window.has_focus() {
            let window_size = self.window.inner_size();
            self.window
                .set_cursor_position(LogicalPosition::new(
                    window_size.width / 2,
                    window_size.height / 2,
                ))
                .unwrap();
        }

        if self
            .input
            .is_key_just_pressed(KeyCode::KeyC)
        {
            self.fly_camera.position.x *= 2.0;
            log::info!("{}", self.fly_camera.position.x);
        }

        // display framerate in window title
        self.window.set_title(&format!(
            "{} ({} fps)",
            WINDOW_TITLE,
            self.time.get_frames_last_second()
        ));

        // update flycam
        if self.fly_camera_active {
            self.fly_camera
                .update(&self.input, &self.time);
        }
        self.render_engine
            .camera_mut()
            .transform = self.fly_camera.get_transform();

        // block breaking and placing (TEMP)
        let destroy = self
            .input
            .is_mouse_button_just_pressed(MouseButton::Left);
        let place_dirt = self
            .input
            .is_key_just_pressed(KeyCode::Digit1);
        let place_grass = self
            .input
            .is_key_just_pressed(KeyCode::Digit2);
        let place_wood = self
            .input
            .is_key_just_pressed(KeyCode::Digit3);
        let place_lamp = self
            .input
            .is_key_just_pressed(KeyCode::Digit4);
        if destroy || place_dirt || place_grass || place_wood || place_lamp {
            let look_dir = self.render_engine.camera().look_dir(); // bad coupling

            let hit = self.terrain.raymarch(
                self.load_area_index,
                self.fly_camera.position,
                look_dir,
                50.0,
            );

            if let Some(hit) = hit {
                if destroy {
                    self.terrain
                        .set_block(self.load_area_index, &hit.hit_pos, BLOCK_AIR);
                }
                if place_dirt {
                    if let Some(hit_normal) = hit.hit_normal {
                        self.terrain.set_block(
                            self.load_area_index,
                            &(hit.hit_pos + GlobalBlockPos::from(hit_normal)),
                            BLOCK_DIRT,
                        );
                    }
                }
                if place_grass {
                    if let Some(hit_normal) = hit.hit_normal {
                        self.terrain.set_block(
                            self.load_area_index,
                            &(hit.hit_pos + GlobalBlockPos::from(hit_normal)),
                            BLOCK_GRASS,
                        );
                    }
                }
                if place_wood {
                    if let Some(hit_normal) = hit.hit_normal {
                        self.terrain.set_block(
                            self.load_area_index,
                            &(hit.hit_pos + GlobalBlockPos::from(hit_normal)),
                            BLOCK_WOOD,
                        );
                    }
                }
                if place_lamp {
                    if let Some(hit_normal) = hit.hit_normal {
                        self.terrain.set_block(
                            self.load_area_index,
                            &(hit.hit_pos + GlobalBlockPos::from(hit_normal)),
                            BLOCK_LAMP_ORANGE,
                        );
                    }
                }
            }
        }

        self.terrain.load_areas_mut()[self.load_area_index]
            .set_center(self.fly_camera.position / (CHUNK_SIZE as f32));

        self.terrain
            .update(&mut self.tasks, self.fly_camera.position);

        self.input.reset();
    }

    fn render(&mut self) {
        let Some(surface_texture) = self
            .render_context
            .get_surface_texture()
        else {
            log::warn!("couldn't acquire surface texture");
            return;
        };

        let output_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.render_engine.render(
            &self.render_context,
            &output_view,
            &self.time,
            &mut self.tasks,
            &self.terrain,
            self.load_area_index,
        );

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
