use d3dx12::*;
use dxsample::*;
use std::convert::TryInto;
use std::mem::transmute;
use windows::{
    runtime::*,
    Win32::{
        Foundation::*,
        Graphics::{
            Direct3D::{*, Fxc::*},
            Direct3D12::*,
            Dxgi::Common::*,
            Dxgi::*,
        },
    },
};

mod d3d12_hello_texture {

    use super::*;

    const FRAME_COUNT: usize = 2;
    const TEXTURE_WIDTH: u64 = 256;
    const TEXTURE_HEIGHT: u32 = 256;

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
        srv_heap: CbvSrvUavDescriptorHeap,
        viewport: D3D12_VIEWPORT,
        scissor_rect: RECT,
        command_allocator: ID3D12CommandAllocator,
        root_signature: ID3D12RootSignature,
        pso: ID3D12PipelineState,
        command_list: ID3D12GraphicsCommandList,
        _vertex_buffer: ID3D12Resource,
        vbv: D3D12_VERTEX_BUFFER_VIEW,
        _texture: ID3D12Resource,
    }

    impl DXSample for Sample {
        fn new(command_line: &SampleCommandLine) -> Result<Self> {
            let (dxgi_factory, device) = create_device(command_line)?;

            Ok(Sample {
                dxgi_factory,
                device,
                resources: None,
            })
        }

        fn bind_to_window(&mut self, hwnd: &HWND) -> Result<()> {
            let mut command_queue =
                SynchronizedCommandQueue::new(&self.device, D3D12_COMMAND_LIST_TYPE_DIRECT)?;

            let (width, height) = self.window_size();

            let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
                BufferCount: FRAME_COUNT as u32,
                Width: width as u32,
                Height: height as u32,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    ..Default::default()
                },
                ..Default::default()
            };

            let swap_chain: IDXGISwapChain3 = unsafe {
                self.dxgi_factory.CreateSwapChainForHwnd(
                    &command_queue.queue,
                    hwnd,
                    &swap_chain_desc,
                    std::ptr::null(),
                    None,
                )
            }?
            .cast()?;

            // This sample does not support fullscreen transitions
            unsafe {
                self.dxgi_factory
                    .MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER)
            }?;

            let frame_index = unsafe { swap_chain.GetCurrentBackBufferIndex() }
                .try_into()
                .unwrap();

            let rtv_heap = RtvDescriptorHeap::new(&self.device, FRAME_COUNT)?;
            let srv_heap = CbvSrvUavDescriptorHeap::new(
                &self.device,
                1,
                D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
            )?;

            let render_targets: [ID3D12Resource; FRAME_COUNT] =
                array_init::try_array_init(|i: usize| -> Result<ID3D12Resource> {
                    let render_target: ID3D12Resource = unsafe { swap_chain.GetBuffer(i as u32) }?;
                    unsafe {
                        rtv_heap.create_render_target_view(&self.device, &render_target, None, i);
                    }
                    Ok(render_target)
                })?;

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
            unsafe { command_list.Close() }?;

            let aspect_ratio = width as f32 / height as f32;

            let (vertex_buffer, vbv) = create_vertex_buffer(&self.device, aspect_ratio)?;

            let texture = create_texture(
                &self.device,
                &mut command_queue,
                &command_allocator,
                &command_list,
            )?;

            unsafe {
                srv_heap.create_shader_resource_view(
                    &self.device,
                    &texture,
                    Some(&D3D12_SHADER_RESOURCE_VIEW_DESC::texture2d(
                        texture.GetDesc().Format,
                        D3D12_TEX2D_SRV {
                            MipLevels: 1,
                            ..Default::default()
                        },
                    )),
                    0,
                );
            }

            self.resources = Some(Resources {
                command_queue,
                swap_chain,
                frame_index,
                render_targets,
                rtv_heap,
                srv_heap,
                viewport,
                scissor_rect,
                command_allocator,
                root_signature,
                pso,
                command_list,
                _vertex_buffer: vertex_buffer,
                vbv,
                _texture: texture,
            });

            Ok(())
        }

        fn title(&self) -> String {
            "D3D12 Hello Texture".into()
        }

        fn window_size(&self) -> (i32, i32) {
            (1280, 720)
        }

        fn render(&mut self) {
            let resources = match &mut self.resources {
                Some(it) => it,
                _ => return,
            };
            populate_command_list(resources).unwrap();

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
    }

    fn populate_command_list(resources: &Resources) -> Result<()> {
        // Command list allocators can only be reset when the associated
        // command lists have finished execution on the GPU; apps should use
        // fences to determine GPU execution progress.
        unsafe { resources.command_allocator.Reset() }?;

        let command_list = &resources.command_list;

        // However, when ExecuteCommandList() is called on a particular
        // command list, that command list can then be reset at any time and
        // must be before re-recording.
        unsafe { command_list.Reset(&resources.command_allocator, &resources.pso) }?;

        // Set necessary state.
        unsafe {
            command_list.SetGraphicsRootSignature(&resources.root_signature);
            let mut heaps = [Some(resources.srv_heap.heap.clone())];
            command_list.SetDescriptorHeaps(heaps.len() as u32, transmute(&mut heaps));
            command_list
                .SetGraphicsRootDescriptorTable(0, resources.srv_heap.get_gpu_descriptor_handle(0));
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

        unsafe { command_list.Close() }
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
            RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
            NumDescriptors: 1,
            BaseShaderRegister: 0,
            RegisterSpace: 0,
            Flags: D3D12_DESCRIPTOR_RANGE_FLAG_DATA_STATIC,
            OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
        }];

        let root_parameters = [D3D12_ROOT_PARAMETER1 {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
            Anonymous: D3D12_ROOT_PARAMETER1_0 {
                DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE1 {
                    NumDescriptorRanges: 1,
                    pDescriptorRanges: unsafe { std::mem::transmute(&ranges) },
                },
            },
        }];

        let sampler = D3D12_STATIC_SAMPLER_DESC {
            Filter: D3D12_FILTER_MIN_MAG_MIP_POINT,
            AddressU: D3D12_TEXTURE_ADDRESS_MODE_BORDER,
            AddressV: D3D12_TEXTURE_ADDRESS_MODE_BORDER,
            AddressW: D3D12_TEXTURE_ADDRESS_MODE_BORDER,
            MipLODBias: 0.0,
            MaxAnisotropy: 0,
            ComparisonFunc: D3D12_COMPARISON_FUNC_NEVER,
            BorderColor: D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK,
            MinLOD: 0.0,
            MaxLOD: D3D12_FLOAT32_MAX,
            ShaderRegister: 0,
            RegisterSpace: 0,
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
        };

        let desc = D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
            Version: D3D_ROOT_SIGNATURE_VERSION_1_1,
            Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
                Desc_1_1: D3D12_ROOT_SIGNATURE_DESC1 {
                    NumParameters: 1,
                    pParameters: unsafe { transmute(&root_parameters) },
                    NumStaticSamplers: 1,
                    pStaticSamplers: unsafe { transmute(&sampler) },
                    Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
                },
            },
        };

        let mut signature = None;

        let signature = unsafe {
            D3D12SerializeVersionedRootSignature(&desc, &mut signature, std::ptr::null_mut())
        }
        .and(Ok(signature.unwrap()))?;

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
        let shaders_hlsl_path = asset_path.join("hello-texture-shaders.hlsl");
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
        .and(Ok(vertex_shader.unwrap()))?;

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
        .and(Ok(pixel_shader.unwrap()))?;

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
                SemanticName: PSTR(b"TEXCOORD\0".as_ptr() as _),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
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
                uv: [0.5, 0.0],
            },
            Vertex {
                position: [0.25, -0.25 * aspect_ratio, 0.0],
                uv: [1.0, 1.0],
            },
            Vertex {
                position: [-0.25, -0.25 * aspect_ratio, 0.0],
                uv: [0.0, 1.0],
            },
        ];

        // Note: using upload heaps to transfer static data like vert buffers is
        // not recommended. Every time the GPU needs it, the upload heap will be
        // marshalled over. Please read up on Default Heap usage. An upload heap
        // is used here for code simplicity and because there are very few verts
        // to actually transfer.
        let mut vertex_buffer = None;
        let vertex_buffer: ID3D12Resource = unsafe {
            device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES::standard(D3D12_HEAP_TYPE_UPLOAD),
                D3D12_HEAP_FLAG_NONE,
                &D3D12_RESOURCE_DESC::buffer(std::mem::size_of_val(&vertices)),
                D3D12_RESOURCE_STATE_GENERIC_READ,
                std::ptr::null(),
                &mut vertex_buffer,
            )
        }
        .and(Ok(vertex_buffer.unwrap()))?;

        // Copy the triangle data to the vertex buffer.
        unsafe {
            let mut data = std::ptr::null_mut();
            vertex_buffer.Map(0, std::ptr::null(), &mut data)?;
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
        uv: [f32; 2],
    }

    fn create_texture(
        device: &ID3D12Device,
        command_queue: &mut SynchronizedCommandQueue,
        command_allocator: &ID3D12CommandAllocator,
        command_list: &ID3D12GraphicsCommandList,
    ) -> Result<ID3D12Resource> {
        let texture_desc =
            D3D12_RESOURCE_DESC::tex2d(DXGI_FORMAT_R8G8B8A8_UNORM, TEXTURE_WIDTH, TEXTURE_HEIGHT);

        let mut texture = None;
        let texture: ID3D12Resource = unsafe {
            device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES::standard(D3D12_HEAP_TYPE_DEFAULT),
                D3D12_HEAP_FLAG_NONE,
                &texture_desc,
                D3D12_RESOURCE_STATE_COPY_DEST,
                std::ptr::null(),
                &mut texture,
            )
        }
        .and(Ok(texture.unwrap()))?;

        let mut placed_subresource_footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
            ..Default::default()
        };
        let mut upload_buffer_size = 0;

        unsafe {
            device.GetCopyableFootprints(
                &texture_desc,
                0,
                1,
                0,
                &mut placed_subresource_footprint,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut upload_buffer_size,
            );
        }

        let mut upload_buffer = None;
        let upload_buffer: ID3D12Resource = unsafe {
            device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES::standard(D3D12_HEAP_TYPE_UPLOAD),
                D3D12_HEAP_FLAG_NONE,
                &D3D12_RESOURCE_DESC::buffer(upload_buffer_size as usize),
                D3D12_RESOURCE_STATE_GENERIC_READ,
                std::ptr::null_mut(),
                &mut upload_buffer,
            )
        }
        .and(Ok(upload_buffer.unwrap()))?;

        unsafe {
            let mut upload_data = std::ptr::null_mut();
            upload_buffer.Map(0, std::ptr::null(), &mut upload_data)?;

            generate_texture_data(
                upload_data.cast(),
                &texture_desc,
                &placed_subresource_footprint.Footprint,
            );

            upload_buffer.Unmap(0, std::ptr::null());
        }

        unsafe {
            command_list.Reset(command_allocator, None)?;
            command_list.CopyTextureRegion(
                &D3D12_TEXTURE_COPY_LOCATION {
                    pResource: Some(texture.clone()),
                    Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        SubresourceIndex: 0,
                    },
                },
                0,
                0,
                0,
                &D3D12_TEXTURE_COPY_LOCATION {
                    pResource: Some(upload_buffer.clone()),
                    Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        PlacedFootprint: placed_subresource_footprint,
                    },
                },
                std::ptr::null(),
            );
            command_list.ResourceBarrier(
                1,
                &transition_barrier(
                    &texture,
                    D3D12_RESOURCE_STATE_COPY_DEST,
                    D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                ),
            );
        }

        unsafe {
            command_list.Close()?;
            command_queue.ExecuteCommandLists(1, &mut Some(ID3D12CommandList::from(command_list)))
        };

        // Wait for the GPU to finish any creation work before returning
        command_queue.signal_and_wait_for_gpu()?;

        // We must wait until the GPU has finished before we can drop the upload
        // buffer. This is needed because clippy encourages you to remove the
        // clone on the last definite use of upload_buffer it sees, which is not
        // good!
        drop(upload_buffer);

        Ok(texture)
    }

    unsafe fn generate_texture_data(
        mut dest: *mut u8,
        desc: &D3D12_RESOURCE_DESC,
        footprint: &D3D12_SUBRESOURCE_FOOTPRINT,
    ) {
        std::assert_eq!(desc.Format, DXGI_FORMAT_R8G8B8A8_UNORM);

        let cell_width = desc.Width >> 3;
        let cell_height = desc.Height >> 3;

        for row in 0..desc.Height {
            let mut p: *mut u32 = dest.cast();
            for x in 0..desc.Width {
                let cell_x = x / cell_width;
                let cell_y = u64::from(row / cell_height);
                if cell_x % 2 == cell_y % 2 {
                    p.write(0x000000FF);
                } else {
                    p.write(0xFFFFFFFF);
                }
                p = p.offset(1);
            }

            dest = dest.offset(footprint.RowPitch as isize);
        }
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
    run_sample::<d3d12_hello_texture::Sample>()?;

    Ok(())
}
