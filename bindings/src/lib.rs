windows::include_bindings!();

use Windows::Win32::Graphics::Direct3D12::*;

unsafe impl Send for ID3D12GraphicsCommandList {}
unsafe impl Sync for ID3D12GraphicsCommandList {}
