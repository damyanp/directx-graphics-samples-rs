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

use bindings::Windows::Win32::Foundation::HWND;
use dxsample::{DXSample, SampleCommandLine, run_sample};
use windows::*;

mod camera;

struct MultithreadingApp {
}

impl DXSample for MultithreadingApp {
    fn new(command_line: &SampleCommandLine) -> Result<Self>
    {
        Ok(MultithreadingApp{})
    }

    fn bind_to_window(&mut self, hwnd: &HWND) -> Result<()> {
        todo!()
    }
}


fn main() -> Result<()> {
    run_sample::<MultithreadingApp>()?;
    Ok(())
}
