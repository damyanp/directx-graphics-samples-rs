//*********************************************************
//
// Copyright (c) Microsoft. All rights reserved.
// This code is licensed under the MIT License (MIT).
// THIS CODE IS PROVIDED *AS IS* WITHOUT WARRANTY OF
// ANY KIND, EITHER EXPRESS OR IMPLIED, INCLUDING ANY
// IMPLIED WARRANTIES OF FITNESS FOR A PARTICULAR
// PURPOSE, MERCHANTABILITY, OR NON-INFRINGEMENT.
//
//*********************************************************

use bindings::Windows::Win32::{
    Foundation::HWND,
    Graphics::Dxgi::{
        DXGIDeclareAdapterRemovalSupport, DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET,
    },
};
use camera::{Camera, ViewAndProjectionMatrices};
use cgmath::{
    point3, vec3, vec4, Deg, InnerSpace, Matrix3, Matrix4, Point3, Rad, SquareMatrix, Transform,
    Vector3, Vector4,
};
use dxsample::{run_sample, DXSample, SampleCommandLine};
use rendering::Renderer;
use timer::Timer;
use windows::*;

mod camera;
mod rendering;
mod timer;

const NUM_CAMERAS: usize = 3;

#[derive(Default)]
struct MultithreadingApp {
    command_line: SampleCommandLine,
    hwnd: HWND,
    state: State,
    renderer: Option<Renderer>,
    input_state: InputState,
    timer: Timer,
}

#[derive(Default)]
pub struct State {
    camera: Camera,
    lights: [LightState; NUM_CAMERAS],
    light_cameras: [Camera; NUM_CAMERAS],
}

#[derive(Default)]
struct InputState {
    right_arrow_pressed: bool,
    left_arrow_pressed: bool,
    up_arrow_pressed: bool,
    down_arrow_pressed: bool,
    animate: bool,
}

struct LightState {
    position: Point3<f32>,
    direction: Vector3<f32>,
    color: Vector4<f32>,
    falloff: Vector4<f32>,
    view: Matrix4<f32>,
    projection: Matrix4<f32>,
}

impl Default for LightState {
    fn default() -> Self {
        LightState {
            position: point3(0.0, 15.0, -30.0),
            direction: vec3(0.0, 0.0, 1.0),
            color: vec4(0.7, 0.7, 0.7, 1.0),
            falloff: vec4(800.0, 1.0, 0.0, 1.0),
            view: Matrix4::identity(),
            projection: Matrix4::identity(),
        }
    }
}

impl DXSample for MultithreadingApp {
    fn new(_command_line: &SampleCommandLine) -> Result<Self> {
        Ok(MultithreadingApp::default())
    }

    fn bind_to_window(&mut self, hwnd: &HWND) -> Result<()> {
        self.hwnd = *hwnd;
        self.create_resources()
    }

    fn update(&mut self) {
        self.timer.tick();

        let frame_time = self.timer.get_elapsed().as_secs_f32();
        let frame_change = Rad(2.0 * frame_time);

        let camera = &mut self.state.camera;
        let input = &mut self.input_state;

        if input.left_arrow_pressed {
            camera.rotate_yaw(-frame_change);
        }
        if input.right_arrow_pressed {
            camera.rotate_yaw(frame_change);
        }
        if input.up_arrow_pressed {
            camera.rotate_pitch(frame_change);
        }
        if input.down_arrow_pressed {
            camera.rotate_pitch(-frame_change);
        }

        if input.animate {            
            let window_size = self.window_size();
            let state = &mut self.state;
            let lights = state.lights.iter_mut();
            let cameras =state.light_cameras.iter_mut();
            let lights_and_cameras = lights.zip(cameras);

            for (i, (light, camera)) in lights_and_cameras.enumerate() {
                let direction = frame_change * -1.0f32.powf(i as f32);
                let position = Matrix3::from_angle_y(direction).transform_point(light.position);

                let eye = light.position;
                let at = point3(0.0, 8.0, 0.0);
                let up = vec3(0.0, 1.0, 0.0);

                light.direction = (at - eye).normalize();

                let ViewAndProjectionMatrices{view, projection} = camera.get_3dview_proj_matrices(
                    Deg(90.0),
                    window_size.0 as f32,
                    window_size.1 as f32,
                );

                *light = LightState {
                    position,
                    view,
                    projection,
                    ..*light
                };

                *camera = Camera { eye, at, up };
            }
        }
    }

    fn render(&mut self) {
        let renderer = match &mut self.renderer {
            Some(it) => it,
            _ => return,
        };

        let r = renderer.render(&self.state);

        let r = match r {
            Err(e) if is_device_removed(&e) => self.create_resources(),
            _ => r,
        };

        r.unwrap();
    }

    fn on_key_up(&mut self, _key: u8) {}

    fn on_key_down(&mut self, _key: u8) {}

    fn title(&self) -> String {
        "D3D12 Multithreading Sample".into()
    }
}

fn is_device_removed(e: &Error) -> bool {
    match e.code() {
        DXGI_ERROR_DEVICE_REMOVED => true,
        DXGI_ERROR_DEVICE_RESET => true,
        _ => false,
    }
}

impl MultithreadingApp {
    fn create_resources(&mut self) -> Result<()> {
        let (width, height) = self.window_size();
        self.renderer = Some(Renderer::new(&self.command_line, &self.hwnd, width as u32, height as u32)?);
        Ok(())
    }
}

fn main() -> Result<()> {
    unsafe { DXGIDeclareAdapterRemovalSupport() }.ok()?;
    run_sample::<MultithreadingApp>()?;
    Ok(())
}
