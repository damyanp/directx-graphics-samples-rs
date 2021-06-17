use bindings::Windows::Win32::{Foundation::HWND, Graphics::Direct3D12::ID3D12Device};
use windows::*;

pub struct Renderer {
    device: ID3D12Device
}

impl Renderer {
    pub fn new(hwnd:&HWND) -> Result<Self> {
        todo!()
    }

    pub fn render(&mut self) -> Result<()> {
        Ok(())
    }
}
