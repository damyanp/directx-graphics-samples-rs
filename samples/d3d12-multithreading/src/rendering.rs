use array_init::try_array_init;
use async_std::task;
use cgmath::{point3, vec3, vec4, Deg, Matrix4, Point3, SquareMatrix, Vector3, Vector4, Zero};
use d3dx12::*;
use dxsample::*;
use static_assertions::const_assert_eq;
use std::{intrinsics::transmute, sync::Arc};
use windows::{
    runtime::*,
    Win32::{
        Foundation::{HWND, RECT},
        Graphics::{Direct3D12::*, Dxgi::Common::*, Dxgi::*},
    },
};

use crate::State;

struct SendableID3D12GraphicsCommandList(ID3D12GraphicsCommandList);

unsafe impl Send for SendableID3D12GraphicsCommandList {}
unsafe impl Sync for SendableID3D12GraphicsCommandList {}

mod squidroom;
use squidroom::*;

const FRAME_COUNT: usize = 2;
const NULL_DESCRIPTOR_COUNT: usize = 2;
const TEXTURE_DESCRIPTOR_COUNT: usize = squidroom::TEXTURE_COUNT;
const PER_FRAME_GPU_DESCRIPTOR_COUNT: usize = 3;
const GPU_DESCRIPTOR_COUNT: usize =
    NULL_DESCRIPTOR_COUNT + TEXTURE_DESCRIPTOR_COUNT + FRAME_COUNT * PER_FRAME_GPU_DESCRIPTOR_COUNT;

pub struct Renderer {
    _device: ID3D12Device,
    viewport: D3D12_VIEWPORT,
    scissor_rect: RECT,
    command_queue: SynchronizedCommandQueue,
    _rtv_descriptor_heap: RtvDescriptorHeap,
    _dsv_descriptor_heap: DsvDescriptorHeap,
    _gpu_descriptor_heap: CbvSrvUavDescriptorHeap,
    frames: Frames,
}

pub struct Frames {
    device: ID3D12Device4, // TODO: do we need the one in Renderer as well?
    current_index: usize,
    swap_chain: IDXGISwapChain3,
    frames: [Frame; FRAME_COUNT],
    idle_command_lists: Vec<ID3D12GraphicsCommandList>,
    command_lists: Vec<ID3D12GraphicsCommandList>,
}

pub struct Frame {
    command_allocators: Vec<ID3D12CommandAllocator>,
    next_command_allocator: usize,
    fence_value: u64,
    render_data: Arc<FrameRenderData>,
}

#[allow(dead_code)]
struct FrameRenderData {
    resources: Arc<Resources>,
    render_target: ID3D12Resource,
    shadow_texture: ID3D12Resource,
    shadow_cb: ID3D12Resource,
    scene_cb: ID3D12Resource,
    shadow_cb_ptr: *mut SceneConstantBuffer,
    scene_cb_ptr: *mut SceneConstantBuffer,
    render_target_view: D3D12_CPU_DESCRIPTOR_HANDLE,
    shadow_depth_view: D3D12_CPU_DESCRIPTOR_HANDLE,
    shadow_cbv_table: D3D12_GPU_DESCRIPTOR_HANDLE,
    scene_srv_table: D3D12_GPU_DESCRIPTOR_HANDLE,
    scene_cbv_table: D3D12_GPU_DESCRIPTOR_HANDLE,
}

#[repr(C, align(256))]
struct SceneConstantBuffer {
    model: Matrix4<f32>,
    view: Matrix4<f32>,
    projection: Matrix4<f32>,
    ambient_color: Vector4<f32>,
    sample_shadow_map: u32,
    _padding: [u32; 3], // must be aligned to be made up of N float4s
    lights: [LightState; NUM_LIGHTS],
}

impl Default for SceneConstantBuffer {
    fn default() -> Self {
        SceneConstantBuffer {
            model: SquareMatrix::identity(),
            view: SquareMatrix::identity(),
            projection: SquareMatrix::identity(),
            ambient_color: Zero::zero(),
            sample_shadow_map: 0,
            _padding: [0; 3],
            lights: [LightState::default(); NUM_LIGHTS],
        }
    }
}

const_assert_eq!(
    std::mem::size_of::<SceneConstantBuffer>()
        % D3D12_CONSTANT_BUFFER_DATA_PLACEMENT_ALIGNMENT as usize,
    0
);

pub const NUM_LIGHTS: usize = 3;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LightState {
    pub position: Point3<f32>,
    pub _pad0: f32,
    pub direction: Vector3<f32>,
    pub _pad1: f32,
    pub color: Vector4<f32>,
    pub falloff: Vector4<f32>,
    pub view: Matrix4<f32>,
    pub projection: Matrix4<f32>,
}

impl Default for LightState {
    fn default() -> Self {
        LightState {
            position: point3(0.0, 15.0, -30.0),
            _pad0: 0.0,
            direction: vec3(0.0, 0.0, 1.0),
            _pad1: 0.0,
            color: vec4(0.7, 0.7, 0.7, 1.0),
            falloff: vec4(800.0, 1.0, 0.0, 1.0),
            view: Matrix4::identity(),
            projection: Matrix4::identity(),
        }
    }
}

unsafe impl Send for FrameRenderData {}
unsafe impl Sync for FrameRenderData {}

impl Renderer {
    pub fn new(
        command_line: &SampleCommandLine,
        hwnd: &HWND,
        width: u32,
        height: u32,
    ) -> Result<Self> {
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
            right: width as i32,
            bottom: height as i32,
        };

        let (factory, device) = dxsample::create_device(&command_line)?;

        let mut command_queue =
            SynchronizedCommandQueue::new(&device, D3D12_COMMAND_LIST_TYPE_DIRECT)?;

        let swap_chain = create_swap_chain(&factory, &command_queue.queue, hwnd, width, height)?;
        let rtv_descriptor_heap = RtvDescriptorHeap::new(&device, FRAME_COUNT)?;
        let dsv_descriptor_heap = DsvDescriptorHeap::new(&device, FRAME_COUNT + 1)?;
        let gpu_descriptor_heap = CbvSrvUavDescriptorHeap::new(
            &device,
            GPU_DESCRIPTOR_COUNT,
            D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
        )?;

        // Create the depth stencil
        let depth_desc = D3D12_RESOURCE_DESC {
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL
                | D3D12_RESOURCE_FLAG_DENY_SHADER_RESOURCE,
            ..D3D12_RESOURCE_DESC::tex2d(DXGI_FORMAT_D32_FLOAT, width as u64, height)
        };

        let mut depth_stencil = None;
        let depth_stencil = unsafe {
            device.CreateCommittedResource(
                &HeapProperties::default(),
                D3D12_HEAP_FLAG_NONE,
                &depth_desc,
                D3D12_RESOURCE_STATE_DEPTH_WRITE,
                &D3D12_CLEAR_VALUE {
                    Format: DXGI_FORMAT_D32_FLOAT,
                    Anonymous: D3D12_CLEAR_VALUE_0 {
                        DepthStencil: D3D12_DEPTH_STENCIL_VALUE {
                            Depth: 1.0,
                            Stencil: 0,
                        },
                    },
                },
                &mut depth_stencil,
            )
        }
        .and(Ok(depth_stencil.unwrap()))?;

        let depth_stencil_view = dsv_descriptor_heap.get_cpu_descriptor_handle(0);
        unsafe {
            device.CreateDepthStencilView(&depth_stencil, std::ptr::null(), depth_stencil_view);
        }

        // Describe and create 2 null SRVs. Null descriptors are needed in order
        // to achieve the effect of an "unbound" resource.
        let null_srv_desc = D3D12_SHADER_RESOURCE_VIEW_DESC::texture2d(
            DXGI_FORMAT_R8G8B8A8_UNORM,
            D3D12_TEX2D_SRV {
                MostDetailedMip: 0,
                MipLevels: 1,
                ..Default::default()
            },
        );

        unsafe {
            for index in 0..2 {
                gpu_descriptor_heap.create_shader_resource_view(
                    &device,
                    None,
                    Some(&null_srv_desc),
                    index,
                );
            }
        }

        let null_srv_table = gpu_descriptor_heap.get_gpu_descriptor_handle(0);

        let resources = Arc::new(Resources::new(
            &device,
            &mut command_queue,
            gpu_descriptor_heap.slice(2),
            null_srv_table,
            depth_stencil,
            depth_stencil_view,
        )?);

        let frames = Frames::new(
            &device,
            swap_chain,
            &rtv_descriptor_heap,
            &dsv_descriptor_heap.slice(1),
            &gpu_descriptor_heap.slice(NULL_DESCRIPTOR_COUNT + TEXTURE_DESCRIPTOR_COUNT),
            resources.clone(),
        )?;

        Ok(Renderer {
            _device: device,
            viewport,
            scissor_rect,
            command_queue,
            _rtv_descriptor_heap: rtv_descriptor_heap,
            _dsv_descriptor_heap: dsv_descriptor_heap,
            _gpu_descriptor_heap: gpu_descriptor_heap,
            frames,
        })
    }

    pub fn render(&mut self, state: &State) -> Result<()> {
        let render_data = self.frames.start_frame(&self.command_queue)?;
        render_data.set_constant_buffers(&self.viewport, state);

        macro_rules! spawn_async_render_task {
            ( $cl:ident $(, $render_data:ident )?, $block:block ) => {{
                let $cl = SendableID3D12GraphicsCommandList(self.frames.get_next_command_list()?);
                $( let $render_data = render_data.clone(); )?
                task::spawn(async move {
                    let $cl = $cl.0;
                    $block
                    Ok::<SendableID3D12GraphicsCommandList, Error>(SendableID3D12GraphicsCommandList($cl))
                })}
            };
        }

        let pre_render = spawn_async_render_task!(cl, render_data, {
            unsafe {
                // Clear the depth stencil buffer in preparation for rendering the shadow map.
                cl.ClearDepthStencilView(
                    render_data.shadow_depth_view,
                    D3D12_CLEAR_FLAG_DEPTH,
                    1.0,
                    0,
                    0,
                    std::ptr::null(),
                );

                // Indicate that the back buffer will be used as a render target.
                cl.ResourceBarrier(
                    1,
                    &transition_barrier(
                        &render_data.render_target,
                        D3D12_RESOURCE_STATE_PRESENT,
                        D3D12_RESOURCE_STATE_RENDER_TARGET,
                    ),
                );

                // Clear the render target and depth stencil.
                cl.ClearRenderTargetView(
                    render_data.render_target_view,
                    [0.0, 0.0, 0.0, 1.0].as_ptr(),
                    0,
                    std::ptr::null(),
                );

                cl.ClearDepthStencilView(
                    render_data.resources.depth_stencil_view,
                    D3D12_CLEAR_FLAG_DEPTH,
                    1.0,
                    0,
                    0,
                    std::ptr::null(),
                );

                cl.Close()?
            }
        });

        const NUM_TASKS: usize = 8;
        let mut shadow_map_render: [_; NUM_TASKS] = try_array_init(|task_index| -> Result<_> {
            let viewport = self.viewport;
            let scissor_rect = self.scissor_rect;
            let task = spawn_async_render_task!(cl, render_data, {
                unsafe {
                    cl.SetPipelineState(&render_data.resources.shadow_map_pso);
                }
                render_data
                    .resources
                    .set_common_pipeline_state(&cl, viewport, scissor_rect);
                render_data.set_shadow_pass_state(&cl);

                unsafe {
                    // Set null SRVs for the diffuse/normal textures.
                    cl.SetGraphicsRootDescriptorTable(0, render_data.resources.null_srv_table);

                    render_data
                        .resources
                        .draw(&cl, task_index, NUM_TASKS, false);

                    cl.Close()?
                }
            });
            Ok(task)
        })?;

        let mid_render = spawn_async_render_task!(cl, render_data, {
            unsafe {
                cl.ResourceBarrier(
                    1,
                    // Transition the shadow map from writeable to readable.
                    &transition_barrier(
                        &render_data.shadow_texture,
                        D3D12_RESOURCE_STATE_DEPTH_WRITE,
                        D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                    ),
                );

                cl.Close()?
            }
        });

        let mut scene_render: [_; NUM_TASKS] = try_array_init(|task_index| -> Result<_> {
            let viewport = self.viewport;
            let scissor_rect = self.scissor_rect;
            let task = spawn_async_render_task!(cl, render_data, {
                unsafe {
                    cl.SetPipelineState(&render_data.resources.scene_pso);
                }
                render_data
                    .resources
                    .set_common_pipeline_state(&cl, viewport, scissor_rect);
                render_data.set_scene_pass_state(&cl);

                unsafe {
                    render_data.resources.draw(&cl, task_index, NUM_TASKS, true);

                    cl.Close()?
                }
            });
            Ok(task)
        })?;

        let post_render = spawn_async_render_task!(cl, {
            unsafe {
                cl.ResourceBarrier(
                    2,
                    [
                        // Transition the shadow map from readable to writeable
                        transition_barrier(
                            &render_data.shadow_texture,
                            D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                            D3D12_RESOURCE_STATE_DEPTH_WRITE,
                        ),
                        // Indicate that the back buffer will now be used to present
                        transition_barrier(
                            &render_data.render_target,
                            D3D12_RESOURCE_STATE_RENDER_TARGET,
                            D3D12_RESOURCE_STATE_PRESENT,
                        ),
                    ]
                    .as_ptr(),
                );
                cl.Close()?
            }
        });

        task::block_on(async {
            self.frames.command_lists.push(pre_render.await?.0);
            for r in shadow_map_render.iter_mut() {
                self.frames.command_lists.push(r.await?.0);
            }
            self.frames.command_lists.push(mid_render.await?.0);
            for r in scene_render.iter_mut() {
                self.frames.command_lists.push(r.await?.0);
            }
            self.frames.command_lists.push(post_render.await?.0);
            Ok::<(), Error>(()) // <-- see https://rust-lang.github.io/async-book/07_workarounds/02_err_in_async_blocks.html
        })?;

        self.frames.end_frame(&mut self.command_queue)?;

        Ok(())
    }
}

fn create_swap_chain(
    factory: &IDXGIFactory4,
    command_queue: &ID3D12CommandQueue,
    hwnd: &HWND,
    width: u32,
    height: u32,
) -> Result<IDXGISwapChain3> {
    let desc = DXGI_SWAP_CHAIN_DESC1 {
        BufferCount: FRAME_COUNT as u32,
        Width: width,
        Height: height,
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        ..Default::default()
    };

    let swap_chain: IDXGISwapChain3 = unsafe {
        factory.CreateSwapChainForHwnd(command_queue, hwnd, &desc, std::ptr::null(), None)
    }?
    .cast()?;

    unsafe { factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER) }?;

    Ok(swap_chain)
}

impl Frames {
    fn new(
        device: &ID3D12Device,
        swap_chain: IDXGISwapChain3,
        rtv_descriptor_heap: &RtvDescriptorHeap,
        dsv_descriptor_heap: &DsvDescriptorHeap,
        gpu_descriptor_heap: &CbvSrvUavDescriptorHeap,
        resources: Arc<Resources>,
    ) -> Result<Frames> {
        let frames = try_array_init(|i| -> Result<Frame> {
            Ok(Frame {
                command_allocators: Default::default(),
                next_command_allocator: Default::default(),
                fence_value: Default::default(),
                render_data: Arc::new(FrameRenderData::new(
                    device,
                    resources.clone(),
                    unsafe { swap_chain.GetBuffer(i as u32)? },
                    rtv_descriptor_heap.get_cpu_descriptor_handle(i),
                    dsv_descriptor_heap.get_cpu_descriptor_handle(i),
                    &gpu_descriptor_heap.slice(i * PER_FRAME_GPU_DESCRIPTOR_COUNT),
                )?),
            })
        })?;

        let device = device.cast()?;

        Ok(Frames {
            device,
            current_index: 0,
            swap_chain,
            frames,
            idle_command_lists: Default::default(),
            command_lists: Default::default(),
        })
    }

    fn start_frame(
        &mut self,
        command_queue: &SynchronizedCommandQueue,
    ) -> Result<Arc<FrameRenderData>> {
        let frame = &mut self.frames[self.current_index];
        frame.start(command_queue)?;
        Ok(frame.render_data.clone())
    }

    fn end_frame(&mut self, command_queue: &mut SynchronizedCommandQueue) -> Result<()> {
        command_queue.execute_command_lists(&self.command_lists);

        unsafe { self.swap_chain.Present(1, 0)? }

        let frame = &mut self.frames[self.current_index];
        frame.end(command_queue)?;

        self.current_index = unsafe { self.swap_chain.GetCurrentBackBufferIndex() } as usize;

        self.idle_command_lists.append(&mut self.command_lists);
        assert_eq!(self.command_lists.len(), 0);

        Ok(())
    }

    fn get_next_command_list(&mut self) -> Result<ID3D12GraphicsCommandList> {
        let command_list = match self.idle_command_lists.pop() {
            Some(command_list) => command_list,
            None => unsafe {
                self.device.CreateCommandList1::<ID3D12GraphicsCommandList>(
                    0,
                    D3D12_COMMAND_LIST_TYPE_DIRECT,
                    D3D12_COMMAND_LIST_FLAG_NONE,
                )
            }?,
        };

        let frame = &mut self.frames[self.current_index];

        unsafe { command_list.Reset(frame.get_command_allocator(&self.device)?, None)? }

        Ok(command_list)
    }
}

impl Frame {
    fn start(&mut self, command_queue: &SynchronizedCommandQueue) -> Result<()> {
        command_queue.wait_for_gpu(self.fence_value)?;
        for ca in &self.command_allocators {
            unsafe { ca.Reset()? }
        }
        self.next_command_allocator = 0;
        Ok(())
    }

    fn end(&mut self, command_queue: &mut SynchronizedCommandQueue) -> Result<()> {
        self.fence_value = command_queue.enqueue_signal()?;
        Ok(())
    }

    fn get_command_allocator(&mut self, device: &ID3D12Device4) -> Result<&ID3D12CommandAllocator> {
        let allocators = &mut self.command_allocators;

        if self.next_command_allocator == allocators.len() {
            allocators
                .push(unsafe { device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT) }?);
        }

        let allocator = &allocators[self.next_command_allocator];
        self.next_command_allocator += 1;

        Ok(allocator)
    }
}

impl FrameRenderData {
    fn new(
        device: &ID3D12Device,
        resources: Arc<Resources>,
        render_target: ID3D12Resource,
        render_target_view: D3D12_CPU_DESCRIPTOR_HANDLE,
        shadow_depth_view: D3D12_CPU_DESCRIPTOR_HANDLE,
        gpu_descriptor_heap: &CbvSrvUavDescriptorHeap,
    ) -> Result<FrameRenderData> {
        let rt_desc = unsafe { render_target.GetDesc() };

        let shadow_texture_desc = D3D12_RESOURCE_DESC {
            Flags: D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL,
            ..ResourceDesc::tex2d(DXGI_FORMAT_R32_TYPELESS, rt_desc.Width, rt_desc.Height)
        };

        let mut shadow_texture = None;
        let shadow_texture = unsafe {
            device.CreateCommittedResource(
                &HeapProperties::default(),
                D3D12_HEAP_FLAG_NONE,
                &shadow_texture_desc,
                D3D12_RESOURCE_STATE_DEPTH_WRITE,
                &D3D12_CLEAR_VALUE {
                    Format: DXGI_FORMAT_D32_FLOAT,
                    Anonymous: D3D12_CLEAR_VALUE_0 {
                        DepthStencil: D3D12_DEPTH_STENCIL_VALUE {
                            Depth: 1.0,
                            Stencil: 0,
                        },
                    },
                },
                &mut shadow_texture,
            )
        }
        .and(Ok(shadow_texture.unwrap()))?;

        let cb_size = std::mem::size_of::<SceneConstantBuffer>();
        let cb_desc = D3D12_RESOURCE_DESC::buffer(cb_size);
        let mut shadow_cb = None;
        let shadow_cb: ID3D12Resource = unsafe {
            device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES::standard(D3D12_HEAP_TYPE_UPLOAD),
                D3D12_HEAP_FLAG_NONE,
                &cb_desc,
                D3D12_RESOURCE_STATE_GENERIC_READ,
                std::ptr::null(),
                &mut shadow_cb,
            )
        }
        .and(Ok(shadow_cb.unwrap()))?;

        let mut scene_cb = None;
        let scene_cb: ID3D12Resource = unsafe {
            device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES::standard(D3D12_HEAP_TYPE_UPLOAD),
                D3D12_HEAP_FLAG_NONE,
                &cb_desc,
                D3D12_RESOURCE_STATE_GENERIC_READ,
                std::ptr::null(),
                &mut scene_cb,
            )
        }
        .and(Ok(scene_cb.unwrap()))?;

        let mut shadow_cb_ptr: *mut SceneConstantBuffer = std::ptr::null_mut();
        let mut scene_cb_ptr: *mut SceneConstantBuffer = std::ptr::null_mut();

        unsafe {
            shadow_cb.Map(0, &D3D12_RANGE::default(), transmute(&mut shadow_cb_ptr))?;
            scene_cb.Map(0, &D3D12_RANGE::default(), transmute(&mut scene_cb_ptr))?;
        }

        let shadow_srv_descriptor_handles = gpu_descriptor_heap.get_descriptor_handles(0);
        let shadow_cbv_descriptor_handles = gpu_descriptor_heap.get_descriptor_handles(1);
        let scene_cbv_descriptor_handles = gpu_descriptor_heap.get_descriptor_handles(2);

        unsafe {
            device.CreateRenderTargetView(&render_target, std::ptr::null(), render_target_view);

            // Note: original sample explicitly creates a DSV_DESC, but it looks
            // like null should work.
            device.CreateDepthStencilView(
                &shadow_texture,
                &D3D12_DEPTH_STENCIL_VIEW_DESC::tex2d(DXGI_FORMAT_D32_FLOAT, 0),
                shadow_depth_view,
            );

            device.CreateShaderResourceView(
                &shadow_texture,
                &D3D12_SHADER_RESOURCE_VIEW_DESC::texture2d(
                    DXGI_FORMAT_R32_FLOAT,
                    D3D12_TEX2D_SRV {
                        MipLevels: 1,
                        ..Default::default()
                    },
                ),
                shadow_srv_descriptor_handles.cpu,
            );

            device.CreateConstantBufferView(
                &D3D12_CONSTANT_BUFFER_VIEW_DESC {
                    BufferLocation: shadow_cb.GetGPUVirtualAddress(),
                    SizeInBytes: cb_size as u32,
                },
                shadow_cbv_descriptor_handles.cpu,
            );

            device.CreateConstantBufferView(
                &D3D12_CONSTANT_BUFFER_VIEW_DESC {
                    BufferLocation: scene_cb.GetGPUVirtualAddress(),
                    SizeInBytes: cb_size as u32,
                },
                scene_cbv_descriptor_handles.cpu,
            );
        }

        Ok(FrameRenderData {
            resources,
            render_target,
            shadow_texture,
            shadow_cb,
            scene_cb,
            shadow_cb_ptr,
            scene_cb_ptr,
            render_target_view,
            shadow_depth_view,
            shadow_cbv_table: shadow_cbv_descriptor_handles.gpu,
            scene_srv_table: shadow_srv_descriptor_handles.gpu,
            scene_cbv_table: scene_cbv_descriptor_handles.gpu,
        })
    }

    fn set_constant_buffers(&self, viewport: &D3D12_VIEWPORT, state: &State) {
        // Scale down the world a bit.
        let scale_down = Matrix4::from_scale(0.1);

        // The scene pass is drawn from the camera.
        let scene_viewproj =
            state
                .camera
                .get_3dview_proj_matrices(Deg(90.0), viewport.Width, viewport.Height);

        // The light pass is drawn from the first light.
        let shadow_viewproj = state.light_cameras[0].get_3dview_proj_matrices(
            Deg(90.0),
            viewport.Width,
            viewport.Height,
        );

        let ambient_color = vec4(0.1, 0.2, 0.3, 1.0);

        let scene_constants = SceneConstantBuffer {
            model: scale_down,
            view: scene_viewproj.view,
            projection: scene_viewproj.projection,
            ambient_color,
            sample_shadow_map: true.into(),
            lights: state.lights,
            ..Default::default()
        };

        let shadow_constants = SceneConstantBuffer {
            model: scale_down,
            view: shadow_viewproj.view,
            projection: shadow_viewproj.projection,
            ambient_color,
            sample_shadow_map: false.into(),
            lights: state.lights,
            ..Default::default()
        };

        unsafe {
            self.scene_cb_ptr.write(scene_constants);
            self.shadow_cb_ptr.write(shadow_constants);
        }
    }

    fn set_shadow_pass_state(&self, cl: &ID3D12GraphicsCommandList) {
        unsafe {
            cl.SetGraphicsRootDescriptorTable(2, self.resources.null_srv_table);
            cl.SetGraphicsRootDescriptorTable(1, self.shadow_cbv_table);

            cl.OMSetRenderTargets(0, std::ptr::null_mut(), false, &self.shadow_depth_view);
        }
    }

    fn set_scene_pass_state(&self, cl: &ID3D12GraphicsCommandList) {
        unsafe {
            cl.SetGraphicsRootDescriptorTable(2, self.scene_srv_table);
            cl.SetGraphicsRootDescriptorTable(1, self.scene_cbv_table);

            cl.OMSetRenderTargets(
                1,
                &self.render_target_view,
                false,
                &self.resources.depth_stencil_view,
            );
        }
    }
}
