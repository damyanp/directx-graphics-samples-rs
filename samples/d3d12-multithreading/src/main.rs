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
    Graphics::{
        Direct3D12::ID3D12Device,
        Dxgi::{
            DXGIDeclareAdapterRemovalSupport, DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET,
        },
    },
};
use dxsample::{run_sample, DXSample, SampleCommandLine};
use windows::*;

mod camera;
mod rendering;
mod timer;

use rendering::*;
use timer::*;

#[derive(Default)]
struct MultithreadingApp {
    hwnd: HWND,
    renderer: Option<Renderer>,
    timer: Timer,
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
    }

    fn render(&mut self) {
        let renderer = match &mut self.renderer {
            Some(it) => it,
            _ => return,
        };

        let r = renderer.render();

        let r = match r {
            Err(e) if is_device_removed(&e) => {
                self.create_resources()
            }            
            _ => r
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
        self.renderer = Some(Renderer::new(&self.hwnd)?);
        Ok(())
    }
}

fn main() -> Result<()> {
    unsafe { DXGIDeclareAdapterRemovalSupport() }.ok()?;
    run_sample::<MultithreadingApp>()?;
    Ok(())
}
