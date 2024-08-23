use windows::Win32::Graphics::{Direct3D12::*, Dxgi::Common::*};

mod descriptor_heaps;
pub use descriptor_heaps::*;

mod pipeline_states;
pub use pipeline_states::*;

pub mod build;

pub fn transition_barrier(
    resource: &ID3D12Resource,
    state_before: D3D12_RESOURCE_STATES,
    state_after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: unsafe { std::mem::transmute_copy(resource) },
                StateBefore: state_before,
                StateAfter: state_after,
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
            }),
        },
    }
}

pub trait ResourceDesc {
    fn default() -> Self;
    fn buffer(size: usize) -> Self;
    fn tex2d(format: DXGI_FORMAT, width: u64, height: u32) -> Self;
}

impl ResourceDesc for D3D12_RESOURCE_DESC {
    fn buffer(size: usize) -> Self {
        D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Width: size as u64,
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            ..ResourceDesc::default()
        }
    }

    fn tex2d(format: DXGI_FORMAT, width: u64, height: u32) -> Self {
        D3D12_RESOURCE_DESC {
            Format: format,
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Width: width,
            Height: height,
            ..ResourceDesc::default()
        }
    }

    fn default() -> Self {
        D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_UNKNOWN,
            Alignment: 0,
            Width: 1,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_UNKNOWN,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: D3D12_RESOURCE_FLAG_NONE,
        }
    }
}

pub trait HeapProperties {
    fn default() -> Self;
    fn standard(heap_type: D3D12_HEAP_TYPE) -> Self;
}

impl HeapProperties for D3D12_HEAP_PROPERTIES {
    fn default() -> Self {
        D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
            MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
            CreationNodeMask: 1,
            VisibleNodeMask: 1,
        }
    }

    fn standard(heap_type: D3D12_HEAP_TYPE) -> Self {
        D3D12_HEAP_PROPERTIES {
            Type: heap_type,
            ..HeapProperties::default()
        }
    }
}
