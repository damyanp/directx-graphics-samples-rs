use bindings::Windows::Win32::Graphics::{Direct3D12::*, Dxgi::DXGI_FORMAT};
use windows::*;

pub trait DescriptorHeap {
    fn create(
        device: &ID3D12Device,
        heap_type: D3D12_DESCRIPTOR_HEAP_TYPE,
        num_descriptors: usize,
        flags: D3D12_DESCRIPTOR_HEAP_FLAGS,
    ) -> Result<Self>
    where
        Self: Sized,
    {
        let heap = unsafe {
            device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: heap_type,
                NumDescriptors: num_descriptors as u32,
                Flags: flags,
                NodeMask: 0,
            })
        }?;

        Ok(Self::from_descriptor_heap(device, heap, heap_type, flags))
    }

    fn from_descriptor_heap(
        device: &ID3D12Device,
        heap: ID3D12DescriptorHeap,
        heap_type: D3D12_DESCRIPTOR_HEAP_TYPE,
        flags: D3D12_DESCRIPTOR_HEAP_FLAGS,
    ) -> Self
    where
        Self: Sized,
    {
        let increment = unsafe { device.GetDescriptorHandleIncrementSize(heap_type) } as usize;
        let start_cpu_handle = unsafe { heap.GetCPUDescriptorHandleForHeapStart() };
        let start_gpu_handle = if flags == D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE {
            unsafe { heap.GetGPUDescriptorHandleForHeapStart() }
        } else {
            D3D12_GPU_DESCRIPTOR_HANDLE { ptr: 0 }
        };

        Self::from_fields(heap, start_cpu_handle, start_gpu_handle, increment)
    }

    fn from_fields(
        heap: ID3D12DescriptorHeap,
        start_cpu_handle: D3D12_CPU_DESCRIPTOR_HANDLE,
        start_gpu_handle: D3D12_GPU_DESCRIPTOR_HANDLE,
        increment: usize,
    ) -> Self;

    fn start_cpu_handle(&self) -> D3D12_CPU_DESCRIPTOR_HANDLE;
    fn start_gpu_handle(&self) -> D3D12_GPU_DESCRIPTOR_HANDLE;
    fn increment(&self) -> usize;

    fn get_cpu_descriptor_handle(&self, index: usize) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        D3D12_CPU_DESCRIPTOR_HANDLE {
            ptr: self.start_cpu_handle().ptr + self.increment() * index,
        }
    }

    fn get_gpu_descriptor_handle(&self, index: usize) -> D3D12_GPU_DESCRIPTOR_HANDLE {
        D3D12_GPU_DESCRIPTOR_HANDLE {
            ptr: self.start_gpu_handle().ptr + (self.increment() * index) as u64,
        }
    }
}

pub struct RtvDescriptorHeap {
    pub heap: ID3D12DescriptorHeap,
    pub start_cpu_handle: D3D12_CPU_DESCRIPTOR_HANDLE,
    pub increment: usize,
}

impl DescriptorHeap for RtvDescriptorHeap {
    fn from_fields(
        heap: ID3D12DescriptorHeap,
        start_cpu_handle: D3D12_CPU_DESCRIPTOR_HANDLE,
        start_gpu_handle: D3D12_GPU_DESCRIPTOR_HANDLE,
        increment: usize,
    ) -> Self {
        std::assert_eq!(start_gpu_handle.ptr, 0);
        RtvDescriptorHeap {
            heap,
            start_cpu_handle,
            increment,
        }
    }

    fn start_cpu_handle(&self) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        self.start_cpu_handle
    }
    fn start_gpu_handle(&self) -> D3D12_GPU_DESCRIPTOR_HANDLE {
        std::panic!();
    }
    fn increment(&self) -> usize {
        self.increment
    }
}

impl RtvDescriptorHeap {
    pub fn new(device: &ID3D12Device, num_descriptors: usize) -> Result<Self> {
        DescriptorHeap::create(
            device,
            D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            num_descriptors,
            D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
        )
    }

    /// Creates an RTV in this heap.
    ///
    /// # Safety
    /// Ensure that dest_index is a valid index in the heap and that the desc is
    /// valid.
    pub unsafe fn create_render_target_view<'a>(
        &self,
        device: &ID3D12Device,
        resource: impl IntoParam<'a, ID3D12Resource>,
        desc: Option<&D3D12_RENDER_TARGET_VIEW_DESC>,
        dest_index: usize,
    ) {
        let desc_ptr: *const D3D12_RENDER_TARGET_VIEW_DESC = if let Some(desc) = desc {
            desc
        } else {
            std::ptr::null()
        };

        device.CreateRenderTargetView(
            resource,
            desc_ptr,
            self.get_cpu_descriptor_handle(dest_index),
        );
    }
}

pub struct DsvDescriptorHeap {
    pub heap: ID3D12DescriptorHeap,
    pub start_cpu_handle: D3D12_CPU_DESCRIPTOR_HANDLE,
    pub increment: usize,
}

impl DescriptorHeap for DsvDescriptorHeap {
    fn from_fields(
        heap: ID3D12DescriptorHeap,
        start_cpu_handle: D3D12_CPU_DESCRIPTOR_HANDLE,
        start_gpu_handle: D3D12_GPU_DESCRIPTOR_HANDLE,
        increment: usize,
    ) -> Self {
        std::assert_eq!(start_gpu_handle.ptr, 0);
        DsvDescriptorHeap {
            heap,
            start_cpu_handle,
            increment,
        }
    }

    fn start_cpu_handle(&self) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        self.start_cpu_handle
    }
    fn start_gpu_handle(&self) -> D3D12_GPU_DESCRIPTOR_HANDLE {
        std::panic!();
    }
    fn increment(&self) -> usize {
        self.increment
    }
}

impl DsvDescriptorHeap {
    pub fn new(device: &ID3D12Device, num_descriptors: usize) -> Result<Self> {
        DescriptorHeap::create(
            device,
            D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
            num_descriptors,
            D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
        )
    }

    /// Creates a DSV in this heap.
    ///
    /// # Safety
    /// Ensure that dest_index is a valid index in the heap and that the desc is
    /// valid.
    pub unsafe fn create_depth_stencil_view<'a>(
        &self,
        device: &ID3D12Device,
        resource: impl IntoParam<'a, ID3D12Resource>,
        desc: Option<&D3D12_DEPTH_STENCIL_VIEW_DESC>,
        dest_index: usize,
    ) {
        let desc_ptr: *const D3D12_DEPTH_STENCIL_VIEW_DESC = if let Some(desc) = desc {
            desc
        } else {
            std::ptr::null()
        };

        device.CreateDepthStencilView(
            resource,
            desc_ptr,
            self.get_cpu_descriptor_handle(dest_index),
        );
    }
}

pub trait DepthStencilViewDesc {
    fn tex2d(format: DXGI_FORMAT, mip_slice: u32) -> Self;
}

impl DepthStencilViewDesc for D3D12_DEPTH_STENCIL_VIEW_DESC {
    fn tex2d(format: DXGI_FORMAT, mip_slice: u32) -> D3D12_DEPTH_STENCIL_VIEW_DESC {
        D3D12_DEPTH_STENCIL_VIEW_DESC {
            Format: format,
            ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D,
            Anonymous: D3D12_DEPTH_STENCIL_VIEW_DESC_0 {
                Texture2D: D3D12_TEX2D_DSV {
                    MipSlice: mip_slice,
                },
            },
            Flags: D3D12_DSV_FLAG_NONE,
        }
    }
}

pub struct CbvSrvUavDescriptorHeap {
    pub heap: ID3D12DescriptorHeap,
    pub start_cpu_handle: D3D12_CPU_DESCRIPTOR_HANDLE,
    pub start_gpu_handle: D3D12_GPU_DESCRIPTOR_HANDLE,
    pub increment: usize,
}

impl DescriptorHeap for CbvSrvUavDescriptorHeap {
    fn from_fields(
        heap: ID3D12DescriptorHeap,
        start_cpu_handle: D3D12_CPU_DESCRIPTOR_HANDLE,
        start_gpu_handle: D3D12_GPU_DESCRIPTOR_HANDLE,
        increment: usize,
    ) -> Self {
        CbvSrvUavDescriptorHeap {
            heap,
            start_cpu_handle,
            start_gpu_handle,
            increment,
        }
    }

    fn start_cpu_handle(&self) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        self.start_cpu_handle
    }

    fn start_gpu_handle(&self) -> D3D12_GPU_DESCRIPTOR_HANDLE {
        self.start_gpu_handle
    }

    fn increment(&self) -> usize {
        self.increment
    }
}

pub struct DescriptorHandles {
    pub cpu: D3D12_CPU_DESCRIPTOR_HANDLE,
    pub gpu: D3D12_GPU_DESCRIPTOR_HANDLE,
}

impl CbvSrvUavDescriptorHeap {
    pub fn new(
        device: &ID3D12Device,
        num_descriptors: usize,
        flags: D3D12_DESCRIPTOR_HEAP_FLAGS,
    ) -> Result<Self> {
        DescriptorHeap::create(
            device,
            D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            num_descriptors,
            flags,
        )
    }

    pub fn slice(&self, start_index: usize) -> Self {
        Self::from_fields(
            self.heap.clone(), // TODO: this clone is icky
            self.get_cpu_descriptor_handle(start_index),
            self.get_gpu_descriptor_handle(start_index),
            self.increment(),
        )
    }

    pub fn get_descriptor_handles(&self, index: usize) -> DescriptorHandles {
        DescriptorHandles {
            cpu: self.get_cpu_descriptor_handle(index),
            gpu: self.get_gpu_descriptor_handle(index),
        }
    }

    /// Creates a SRV in this heap.
    ///
    /// # Safety
    /// Ensure that dest_index is a valid index in the heap and that the desc is
    /// valid.
    pub unsafe fn create_shader_resource_view<'a>(
        &self,
        device: &ID3D12Device,
        resource: impl IntoParam<'a, ID3D12Resource>,
        desc: Option<&D3D12_SHADER_RESOURCE_VIEW_DESC>,
        dest_index: usize,
    ) {
        let desc_ptr: *const D3D12_SHADER_RESOURCE_VIEW_DESC = if let Some(desc) = desc {
            desc
        } else {
            std::ptr::null()
        };

        device.CreateShaderResourceView(
            resource,
            desc_ptr,
            self.get_cpu_descriptor_handle(dest_index),
        );
    }

    /// Creates a CBV in this heap
    ///
    /// # Safety
    /// Ensure that dest_index is a valid index in the heap and that the desc is
    /// valid.
    pub unsafe fn create_constant_buffer_view(
        &self,
        device: &ID3D12Device,
        desc: &D3D12_CONSTANT_BUFFER_VIEW_DESC,
        dest_index: usize,
    ) {
        device.CreateConstantBufferView(desc, self.get_cpu_descriptor_handle(dest_index));
    }
}

pub trait ShaderResourceViewDesc {
    fn texture2d(format: DXGI_FORMAT, srv: D3D12_TEX2D_SRV) -> Self;
}

impl ShaderResourceViewDesc for D3D12_SHADER_RESOURCE_VIEW_DESC {
    fn texture2d(format: DXGI_FORMAT, srv: D3D12_TEX2D_SRV) -> Self {
        D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: format,
            ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
            Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
            Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: srv },
        }
    }
}

pub trait ConstantBufferViewDesc {
    fn entire_resource(resource: &ID3D12Resource) -> Self;
}

impl ConstantBufferViewDesc for D3D12_CONSTANT_BUFFER_VIEW_DESC {
    fn entire_resource(resource: &ID3D12Resource) -> Self {
        unsafe {
            D3D12_CONSTANT_BUFFER_VIEW_DESC {
                BufferLocation: resource.GetGPUVirtualAddress(),
                SizeInBytes: resource.GetDesc().Width as u32,
            }
        }
    }
}
