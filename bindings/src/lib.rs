windows::include_bindings!();

unsafe impl Send for Windows::Win32::Graphics::Direct3D12::ID3D12GraphicsCommandList{}
unsafe impl Sync for Windows::Win32::Graphics::Direct3D12::ID3D12GraphicsCommandList{}

