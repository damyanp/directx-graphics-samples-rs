use bindings::Windows::Win32::{
    Foundation::*,
    Graphics::{Direct3D11::*, Direct3D12::*, Dxgi::*, Hlsl::*},
};
use d3dx12::*;
use dxsample::*;
use std::convert::TryInto;
use windows::*;

extern crate static_assertions as sa;

mod d3d12_hello_constbuffers {

    use std::intrinsics::transmute;

    use super::*;

    const FRAME_COUNT: usize = 2;

    pub struct Sample {
        dxgi_factory: IDXGIFactory4,
        device: ID3D12Device,
        resources: Option<Resources>,
    }

    struct Resources {
        command_queue: SynchronizedCommandQueue,
        swap_chain: IDXGISwapChain3,
        frame_index: usize,
        render_targets: [ID3D12Resource; FRAME_COUNT],
        rtv_heap: RtvDescriptorHeap,
        cbv_heap: CbvSrvUavDescriptorHeap,
        viewport: D3D12_VIEWPORT,
        scissor_rect: RECT,
        command_allocator: ID3D12CommandAllocator,
        root_signature: ID3D12RootSignature,
        pso: ID3D12PipelineState,
        command_list: ID3D12GraphicsCommandList,

        // we need to keep this around to keep the reference alive, even though
        // nothing reads from it
        #[allow(dead_code)]
        vertex_buffer: ID3D12Resource,

        constant_buffer: ConstantBuffer<SceneConstantBuffer>,

        vbv: D3D12_VERTEX_BUFFER_VIEW,
    }

    #[repr(C, align(256))]
    struct SceneConstantBuffer {
        offset: [f32; 4],
    }

    sa::const_assert_eq!(
        std::mem::size_of::<SceneConstantBuffer>()
            % D3D12_CONSTANT_BUFFER_DATA_PLACEMENT_ALIGNMENT as usize,
        0
    );

    struct ConstantBuffer<T> {
        pub resource: ID3D12Resource,
        pub data: T,
        mapped: *mut T,
    }

    impl<T> ConstantBuffer<T> {
        pub fn new(device: &ID3D12Device, initial_data: T) -> Result<Self> {
            let resource: ID3D12Resource = unsafe {
                device.CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES::standard(D3D12_HEAP_TYPE_UPLOAD),
                    D3D12_HEAP_FLAG_NONE,
                    &D3D12_RESOURCE_DESC::buffer(std::mem::size_of::<T>()),
                    D3D12_RESOURCE_STATE_GENERIC_READ,
                    std::ptr::null(),
                )
            }?;

            let mut mapped = std::ptr::null_mut();
            unsafe {
                // We're going to this mapped for the duration of the process.
                resource.Map(0, std::ptr::null(), &mut mapped).ok()?;
            }

            let mapped = mapped as *mut T;
            unsafe {
                std::ptr::copy_nonoverlapping(&initial_data, mapped, 1);
            }

            Ok(ConstantBuffer {
                resource,
                data: initial_data,
                mapped,
            })
        }

        pub fn update(&self) {
            unsafe {
                std::ptr::copy_nonoverlapping(&self.data, self.mapped, 1);
            }
        }
    }

    impl DXSample for Sample {
        fn new(command_line: &SampleCommandLine) -> Result<Self> {
            let (dxgi_factory, device) = create_device(&command_line)?;

            Ok(Sample {
                dxgi_factory,
                device,
                resources: None,
            })
        }

        fn bind_to_window(&mut self, hwnd: &HWND) -> Result<()> {
            let command_queue =
                SynchronizedCommandQueue::new(&self.device, D3D12_COMMAND_LIST_TYPE_DIRECT)?;

            let (width, height) = self.window_size();

            let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: width as u32,
                Height: height as u32,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    ..Default::default()
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: FRAME_COUNT.try_into().unwrap(),
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                ..Default::default()
            };

            let mut swap_chain = None;
            let swap_chain: IDXGISwapChain3 = unsafe {
                self.dxgi_factory.CreateSwapChainForHwnd(
                    &command_queue.queue,
                    hwnd,
                    &swap_chain_desc,
                    std::ptr::null(),
                    None,
                    &mut swap_chain,
                )
            }
            .and_some(swap_chain)?
            .cast()?;

            // This sample does not support fullscreen transitions
            unsafe {
                self.dxgi_factory
                    .MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER)
            }
            .ok()?;

            let frame_index = unsafe { swap_chain.GetCurrentBackBufferIndex() }
                .try_into()
                .unwrap();

            let rtv_heap = RtvDescriptorHeap::new(&self.device, FRAME_COUNT)?;

            let render_targets: [ID3D12Resource; FRAME_COUNT] =
                array_init::try_array_init(|i: usize| -> Result<ID3D12Resource> {
                    let render_target: ID3D12Resource = unsafe { swap_chain.GetBuffer(i as u32) }?;
                    unsafe {
                        rtv_heap.create_render_target_view(&self.device, &render_target, None, i);
                    }
                    Ok(render_target)
                })?;

            let constant_buffer = ConstantBuffer::<SceneConstantBuffer>::new(
                &self.device,
                SceneConstantBuffer {
                    offset: [0.0, 0.0, 0.0, 0.0],
                },
            )?;

            let cbv_heap = CbvSrvUavDescriptorHeap::new(
                &self.device,
                1,
                D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
            )?;

            unsafe {
                cbv_heap.create_constant_buffer_view(
                    &self.device,
                    &D3D12_CONSTANT_BUFFER_VIEW_DESC::entire_resource(&constant_buffer.resource),
                    0,
                );
            }

            let viewport = D3D12_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: width as f32,
                Height: height as f32,
                MinDepth: D3D12_MIN_DEPTH,
                MaxDepth: D3D12_MAX_DEPTH,
            };

            let scissor_rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };

            let command_allocator = unsafe {
                self.device
                    .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
            }?;

            let root_signature = create_root_signature(&self.device)?;
            let pso = create_pipeline_state(&self.device, &root_signature)?;

            let command_list: ID3D12GraphicsCommandList = unsafe {
                self.device.CreateCommandList(
                    0,
                    D3D12_COMMAND_LIST_TYPE_DIRECT,
                    &command_allocator,
                    &pso,
                )
            }?;
            unsafe { command_list.Close() }.ok()?;

            let aspect_ratio = width as f32 / height as f32;

            let (vertex_buffer, vbv) = create_vertex_buffer(&self.device, aspect_ratio)?;

            self.resources = Some(Resources {
                command_queue,
                swap_chain,
                frame_index,
                render_targets,
                rtv_heap,
                cbv_heap,
                viewport,
                scissor_rect,
                command_allocator,
                root_signature,
                pso,
                command_list,
                vertex_buffer,
                constant_buffer,
                vbv,
            });

            Ok(())
        }

        fn render(&mut self) {
            let resources = match &mut self.resources {
                Some(it) => it,
                _ => return,
            };

            const TRANSLATION_SPEED: f32 = 0.005;
            const OFFSET_BOUNDS: f32 = 1.25;

            let offset = &mut resources.constant_buffer.data.offset;
            offset[0] += TRANSLATION_SPEED;
            if offset[0] > OFFSET_BOUNDS {
                offset[0] = -OFFSET_BOUNDS;
            }
            resources.constant_buffer.update();

            populate_command_list(&resources).unwrap();

            // Execute the command list.
            let command_list = ID3D12CommandList::from(&resources.command_list);
            unsafe {
                resources
                    .command_queue
                    .ExecuteCommandLists(1, &mut Some(command_list))
            };

            // Present the frame.
            unsafe { resources.swap_chain.Present(1, 0) }.ok().unwrap();

            wait_for_previous_frame(resources);
        }

        fn title(&self) -> String {
            "D3D12 Hello Constant Buffers".into()
        }

        fn window_size(&self) -> (i32, i32) {
            (1280, 720)
        }
    }

    fn populate_command_list(resources: &Resources) -> Result<()> {
        // Command list allocators can only be reset when the associated
        // command lists have finished execution on the GPU; apps should use
        // fences to determine GPU execution progress.
        unsafe { resources.command_allocator.Reset() }.ok()?;

        let command_list = &resources.command_list;

        // However, when ExecuteCommandList() is called on a particular
        // command list, that command list can then be reset at any time and
        // must be before re-recording.
        unsafe { command_list.Reset(&resources.command_allocator, &resources.pso) }.ok()?;

        // Set necessary state.
        unsafe {
            command_list.SetGraphicsRootSignature(&resources.root_signature);
            let mut heaps = [Some(resources.cbv_heap.heap.clone())];
            command_list.SetDescriptorHeaps(heaps.len().try_into().unwrap(), transmute(&mut heaps));
            command_list.SetGraphicsRootDescriptorTable(0, resources.cbv_heap.start_gpu_handle());
            command_list.RSSetViewports(1, &resources.viewport);
            command_list.RSSetScissorRects(1, &resources.scissor_rect);
        }

        // Indicate that the back buffer will be used as a render target.
        let barrier = transition_barrier(
            &resources.render_targets[resources.frame_index as usize],
            D3D12_RESOURCE_STATE_PRESENT,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        );
        unsafe { command_list.ResourceBarrier(1, &barrier) };

        let rtv_handle = resources
            .rtv_heap
            .get_cpu_descriptor_handle(resources.frame_index);

        unsafe { command_list.OMSetRenderTargets(1, &rtv_handle, false, std::ptr::null()) };

        // Record commands.
        unsafe {
            command_list.ClearRenderTargetView(
                rtv_handle,
                [0.0, 0.2, 0.4, 1.0].as_ptr(),
                0,
                std::ptr::null(),
            );
            command_list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            command_list.IASetVertexBuffers(0, 1, &resources.vbv);
            command_list.DrawInstanced(3, 1, 0, 0);

            // Indicate that the back buffer will now be used to present.
            command_list.ResourceBarrier(
                1,
                &transition_barrier(
                    &resources.render_targets[resources.frame_index as usize],
                    D3D12_RESOURCE_STATE_RENDER_TARGET,
                    D3D12_RESOURCE_STATE_PRESENT,
                ),
            );
        }

        unsafe { command_list.Close() }.ok()
    }

    fn create_root_signature(device: &ID3D12Device) -> Result<ID3D12RootSignature> {
        let mut feature_data = D3D12_FEATURE_DATA_ROOT_SIGNATURE {
            HighestVersion: D3D_ROOT_SIGNATURE_VERSION_1_1,
        };

        if unsafe {
            device.CheckFeatureSupport(
                D3D12_FEATURE_ROOT_SIGNATURE,
                std::mem::transmute(&feature_data),
                std::mem::size_of_val(&feature_data) as u32,
            )
        }
        .is_err()
        {
            feature_data.HighestVersion = D3D_ROOT_SIGNATURE_VERSION_1_0;
        }

        // Although the C++ sample jumps through hoops to run on versions of d3d
        // that don't support 1.1 root signatures, this one does not.
        std::assert_ne!(feature_data.HighestVersion, D3D_ROOT_SIGNATURE_VERSION_1_0);

        let ranges = [D3D12_DESCRIPTOR_RANGE1 {
            RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_CBV,
            NumDescriptors: 1,
            BaseShaderRegister: 0,
            RegisterSpace: 0,
            Flags: D3D12_DESCRIPTOR_RANGE_FLAG_DATA_STATIC,
            OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
        }];

        let root_parameters = [D3D12_ROOT_PARAMETER1 {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
            ShaderVisibility: D3D12_SHADER_VISIBILITY_VERTEX,
            Anonymous: D3D12_ROOT_PARAMETER1_0 {
                DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE1 {
                    NumDescriptorRanges: 1,
                    pDescriptorRanges: unsafe { std::mem::transmute(&ranges) },
                },
            },
        }];

        let desc = D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
            Version: D3D_ROOT_SIGNATURE_VERSION_1_1,
            Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
                Desc_1_1: D3D12_ROOT_SIGNATURE_DESC1 {
                    NumParameters: 1,
                    pParameters: unsafe { transmute(&root_parameters) },
                    Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
                    ..Default::default()
                },
            },
        };

        let mut signature = None;

        let signature = unsafe {
            D3D12SerializeVersionedRootSignature(&desc, &mut signature, std::ptr::null_mut())
        }
        .and_some(signature)?;

        unsafe {
            device.CreateRootSignature(0, signature.GetBufferPointer(), signature.GetBufferSize())
        }
    }

    fn create_pipeline_state(
        device: &ID3D12Device,
        root_signature: &ID3D12RootSignature,
    ) -> Result<ID3D12PipelineState> {
        let compile_flags = if cfg!(debug_assertions) {
            D3DCOMPILE_DEBUG | D3DCOMPILE_SKIP_OPTIMIZATION
        } else {
            0
        };

        let exe_path = std::env::current_exe().ok().unwrap();
        let asset_path = exe_path.parent().unwrap();
        let shaders_hlsl_path = asset_path.join("hello-constbuffers-shaders.hlsl");
        let shaders_hlsl = shaders_hlsl_path.to_str().unwrap();

        let mut vertex_shader = None;
        let vertex_shader = unsafe {
            D3DCompileFromFile(
                shaders_hlsl,
                std::ptr::null_mut(),
                None,
                "VSMain",
                "vs_5_0",
                compile_flags,
                0,
                &mut vertex_shader,
                std::ptr::null_mut(),
            )
        }
        .and_some(vertex_shader)?;

        let mut pixel_shader = None;
        let pixel_shader = unsafe {
            D3DCompileFromFile(
                shaders_hlsl,
                std::ptr::null_mut(),
                None,
                "PSMain",
                "ps_5_0",
                compile_flags,
                0,
                &mut pixel_shader,
                std::ptr::null_mut(),
            )
        }
        .and_some(pixel_shader)?;

        let mut input_element_descs: [D3D12_INPUT_ELEMENT_DESC; 2] = [
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PSTR(b"POSITION\0".as_ptr() as _),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32B32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 0,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PSTR(b"COLOR\0".as_ptr() as _),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 12,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
        ];

        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: input_element_descs.as_mut_ptr(),
                NumElements: input_element_descs.len() as u32,
            },
            pRootSignature: Some(root_signature.clone()), // << https://github.com/microsoft/windows-rs/discussions/623
            VS: D3D12_SHADER_BYTECODE::from_blob(&vertex_shader),
            PS: D3D12_SHADER_BYTECODE::from_blob(&pixel_shader),
            RasterizerState: D3D12_RASTERIZER_DESC::reasonable_default(),
            BlendState: D3D12_BLEND_DESC::reasonable_default(),
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC::default(),
            SampleMask: u32::max_value(),
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;

        unsafe { device.CreateGraphicsPipelineState(&desc) }
    }

    fn create_vertex_buffer(
        device: &ID3D12Device,
        aspect_ratio: f32,
    ) -> Result<(ID3D12Resource, D3D12_VERTEX_BUFFER_VIEW)> {
        let vertices = [
            Vertex {
                position: [0.0, 0.25 * aspect_ratio, 0.0],
                color: [1.0, 0.0, 0.0, 1.0],
            },
            Vertex {
                position: [0.25, -0.25 * aspect_ratio, 0.0],
                color: [0.0, 1.0, 0.0, 1.0],
            },
            Vertex {
                position: [-0.25, -0.25 * aspect_ratio, 0.0],
                color: [0.0, 0.0, 1.0, 1.0],
            },
        ];

        // Note: using upload heaps to transfer static data like vert buffers is
        // not recommended. Every time the GPU needs it, the upload heap will be
        // marshalled over. Please read up on Default Heap usage. An upload heap
        // is used here for code simplicity and because there are very few verts
        // to actually transfer.
        let vertex_buffer: ID3D12Resource = unsafe {
            device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES::standard(D3D12_HEAP_TYPE_UPLOAD),
                D3D12_HEAP_FLAG_NONE,
                &D3D12_RESOURCE_DESC::buffer(std::mem::size_of_val(&vertices)),
                D3D12_RESOURCE_STATE_GENERIC_READ,
                std::ptr::null(),
            )?
        };

        // Copy the triangle data to the vertex buffer.
        unsafe {
            let mut data = std::ptr::null_mut();
            vertex_buffer.Map(0, std::ptr::null(), &mut data).ok()?;
            std::ptr::copy_nonoverlapping(
                vertices.as_ptr(),
                data as *mut Vertex,
                std::mem::size_of_val(&vertices),
            );
            vertex_buffer.Unmap(0, std::ptr::null());
        }

        let vbv = D3D12_VERTEX_BUFFER_VIEW {
            BufferLocation: unsafe { vertex_buffer.GetGPUVirtualAddress() },
            StrideInBytes: std::mem::size_of::<Vertex>() as u32,
            SizeInBytes: std::mem::size_of_val(&vertices) as u32,
        };

        Ok((vertex_buffer, vbv))
    }

    #[repr(C)]
    struct Vertex {
        position: [f32; 3],
        color: [f32; 4],
    }

    fn wait_for_previous_frame(resources: &mut Resources) {
        // WAITING FOR THE FRAME TO COMPLETE BEFORE CONTINUING IS NOT BEST
        // PRACTICE. This is code implemented as such for simplicity. The
        // D3D12HelloFrameBuffering sample illustrates how to use fences for
        // efficient resource usage and to maximize GPU utilization.

        resources.command_queue.signal_and_wait_for_gpu().unwrap();
        resources.frame_index = unsafe { resources.swap_chain.GetCurrentBackBufferIndex() }
            .try_into()
            .unwrap();
    }
}

fn main() -> Result<()> {
    run_sample::<d3d12_hello_constbuffers::Sample>()?;

    Ok(())
}
