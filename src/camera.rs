use std::time::Duration;

use winit::{event::ElementState, keyboard::KeyCode};

#[derive(Debug)]
pub struct Camera {
    pub position: nalgebra::Point3<f32>,
    yaw: f32,
    pitch: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self::new(nalgebra::Point3::new(0.0, 0.0, 0.0), 0.0, 0.0)
    }
}

const SAFE_FRAC_PI_2: f32 = std::f32::consts::FRAC_PI_2 - 0.0001;

impl Camera {
    pub fn new(position: nalgebra::Point3<f32>, yaw: f32, pitch: f32) -> Self {
        Self {
            position,
            yaw,
            pitch,
        }
    }

    fn view_proj_matrix(&self, projection: &CameraProjection) -> nalgebra::Matrix4<f32> {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();

        let view = nalgebra::Matrix4::look_at_rh(
            &self.position,
            &(self.position
                + nalgebra::Vector3::new(cos_yaw * cos_pitch, sin_pitch, sin_yaw * cos_pitch)),
            &nalgebra::Vector3::y(),
        );

        projection.as_matrix() * view
    }
}

pub type CameraProjection = nalgebra::Perspective3<f32>;

#[derive(Debug)]
pub struct CameraController {
    amount_left: f32,
    amount_right: f32,
    amount_forward: f32,
    amount_backward: f32,
    amount_up: f32,
    amount_down: f32,
    rotate_horizontal: f32,
    rotate_vertical: f32,
    speed: f32,
    sensitivity: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self::new(4.0, 0.4)
    }
}

impl CameraController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            amount_left: 0.0,
            amount_right: 0.0,
            amount_forward: 0.0,
            amount_backward: 0.0,
            amount_up: 0.0,
            amount_down: 0.0,
            rotate_horizontal: 0.0,
            rotate_vertical: 0.0,
            speed,
            sensitivity,
        }
    }

    pub fn process_keyboard(&mut self, key: KeyCode, state: ElementState) -> bool {
        let amount = if state == ElementState::Pressed {
            1.0
        } else {
            0.0
        };
        match key {
            KeyCode::KeyW | KeyCode::ArrowUp => {
                self.amount_forward = amount;
            }
            KeyCode::KeyS | KeyCode::ArrowDown => {
                self.amount_backward = amount;
            }
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                self.amount_left = amount;
            }
            KeyCode::KeyD | KeyCode::ArrowRight => {
                self.amount_right = amount;
            }
            KeyCode::Space => {
                self.amount_up = amount;
            }
            KeyCode::ShiftLeft => {
                self.amount_down = amount;
            }
            _ => return false,
        };
        true
    }

    pub fn process_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.rotate_horizontal = mouse_dx as f32;
        self.rotate_vertical = mouse_dy as f32;
    }

    fn update_camera(&mut self, camera: &mut Camera, delta_time: Duration) {
        let dt = delta_time.as_secs_f32();

        // Move forward/backward and left/right
        let (yaw_sin, yaw_cos) = camera.yaw.sin_cos();
        let forward = nalgebra::Vector3::new(yaw_cos, 0.0, yaw_sin).normalize();
        let right = nalgebra::Vector3::new(-yaw_sin, 0.0, yaw_cos).normalize();
        camera.position += forward * (self.amount_forward - self.amount_backward) * self.speed * dt;
        camera.position += right * (self.amount_right - self.amount_left) * self.speed * dt;

        // Move up/down. Since we don't use roll, we can just
        // modify the y coordinate directly.
        camera.position.y += (self.amount_up - self.amount_down) * self.speed * dt;

        // Rotate
        camera.yaw += self.rotate_horizontal * self.sensitivity * dt;
        camera.pitch += -self.rotate_vertical * self.sensitivity * dt;

        // If process_mouse isn't called every frame, these values
        // will not get set to zero, and the camera will rotate
        // when moving in a non cardinal direction.
        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;

        // Keep the camera's angle from going too high/low.
        if camera.pitch < -SAFE_FRAC_PI_2 {
            camera.pitch = -SAFE_FRAC_PI_2;
        } else if camera.pitch > SAFE_FRAC_PI_2 {
            camera.pitch = SAFE_FRAC_PI_2;
        }
    }
}

#[derive(Debug)]
pub struct CameraModule {
    pub camera: Camera,
    projection: CameraProjection,
    pub controller: CameraController,
}

impl CameraModule {
    pub fn from_parts(
        camera: Camera,
        projection: CameraProjection,
        controller: CameraController,
    ) -> Self {
        Self {
            camera,
            projection,
            controller,
        }
    }

    pub fn update(&mut self, delta_time: Duration) {
        self.controller.update_camera(&mut self.camera, delta_time);
    }

    pub fn view_proj_matrix(&self) -> nalgebra::Matrix4<f32> {
        self.camera.view_proj_matrix(&self.projection)
    }
}
