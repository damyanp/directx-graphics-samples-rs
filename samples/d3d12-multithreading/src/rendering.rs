use array_init::try_array_init;
use async_std::task;
use bindings::Windows::Win32::{
    Foundation::HWND,
    Graphics::{Direct3D12::*, Dxgi::*},
};
use d3dx12::*;
use dxsample::*;
use std::sync::Arc;
use windows::*;

use crate::State;

const FRAME_COUNT: usize = 2;

pub struct Renderer {
    _device: ID3D12Device,
    command_queue: SynchronizedCommandQueue,
    _swap_chain: SwapChain,
    frames: Frames,
}

unsafe impl Send for RenderData {}
unsafe impl Sync for RenderData {}

pub struct SwapChain {
    _dxgi_swap_chain: IDXGISwapChain3,
    _render_targets: [ID3D12Resource; FRAME_COUNT],
    rtv_heap: RtvDescriptorHeap,
    dsv_heap: DsvDescriptorHeap,
}

pub struct Frames {
    device: ID3D12Device4, // TODO: do we need the one in Renderer as well?
    current_index: usize,
    frames: [Frame; FRAME_COUNT],
    idle_command_lists: Vec<ID3D12GraphicsCommandList>,
    command_lists: Vec<ID3D12GraphicsCommandList>,
}

pub struct Frame {
    command_allocators: Vec<ID3D12CommandAllocator>,
    next_command_allocator: usize,
    fence_value: u64,
    render_data: Arc<RenderData>,
}

struct RenderData {
    render_target_view: D3D12_CPU_DESCRIPTOR_HANDLE,
    shadow_depth_view: D3D12_CPU_DESCRIPTOR_HANDLE,
}

impl Renderer {
    pub fn new(
        command_line: &SampleCommandLine,
        hwnd: &HWND,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let (factory, device) = dxsample::create_device(&command_line)?;

        let command_queue = SynchronizedCommandQueue::new(&device, D3D12_COMMAND_LIST_TYPE_DIRECT)?;

        let swap_chain = SwapChain::new(&factory, &device, &command_queue, hwnd, width, height)?;

        let frames = Frames::new(&device, &swap_chain)?;

        Ok(Renderer {
            _device: device,
            command_queue,
            _swap_chain: swap_chain,
            frames,
        })
    }

    pub fn render(&mut self, _state: &State) -> Result<()> {
        let render_data = self.frames.start_frame(&self.command_queue)?;

        macro_rules! spawn_async_render_task {
            ( $cl:ident $(, $render_data:ident )?, $block:block ) => {{
                let $cl = self.frames.get_next_command_list()?;
                $( let $render_data = render_data.clone(); )?
                task::spawn(async move {
                    $block
                    Ok::<ID3D12GraphicsCommandList, Error>($cl)
                })}
            };
        }

        let pre_render = spawn_async_render_task!(cl, render_data, {
            unsafe {
                cl.ClearDepthStencilView(
                    render_data.shadow_depth_view,
                    D3D12_CLEAR_FLAG_DEPTH,
                    1.0,
                    0,
                    0,
                    std::ptr::null(),
                );

                // TODO: transition back buffer

                // clear rtv & depth stencil
                cl.ClearRenderTargetView(
                    render_data.render_target_view,
                    [0.0, 0.0, 0.0, 1.0].as_ptr(),
                    0,
                    std::ptr::null(),
                );

                cl.ClearDepthStencilView(
                    D3D12_CPU_DESCRIPTOR_HANDLE { ptr: 0 }, // TODO
                    D3D12_CLEAR_FLAG_DEPTH,
                    1.0,
                    0,
                    0,
                    std::ptr::null(),
                );

                cl.Close().ok()?
            }
        });

        let post_render =
            spawn_async_render_task!(cl, render_data, { unsafe { cl.Close().ok()? } });

        task::block_on(async {
            self.frames.command_lists.push(pre_render.await?);
            self.frames.command_lists.push(post_render.await?);
            Ok::<(), Error>(()) // <-- see https://rust-lang.github.io/async-book/07_workarounds/02_err_in_async_blocks.html
        })?;

        //let cl = self.frames.get_command_list()?;
        //let post_render = self.post_render(cl);

        //let render_futures = join(pre_render, post_render);

        self.frames.end_frame(&mut self.command_queue)?;
        Ok(())
    }
}

impl SwapChain {
    fn new(
        factory: &IDXGIFactory4,
        device: &ID3D12Device,
        command_queue: &SynchronizedCommandQueue,
        hwnd: &HWND,
        width: u32,
        height: u32,
    ) -> Result<SwapChain> {
        let dxgi_swap_chain =
            create_swap_chain(factory, &command_queue.queue, hwnd, width, height)?;

        let rtv_heap = RtvDescriptorHeap::new(device, FRAME_COUNT)?;
        let dsv_heap = DsvDescriptorHeap::new(device, FRAME_COUNT)?;

        let render_targets = try_array_init(|i| -> Result<ID3D12Resource> {
            let render_target: ID3D12Resource = unsafe { dxgi_swap_chain.GetBuffer(i as u32) }?;
            unsafe {
                rtv_heap.create_render_target_view(device, &render_target, None, i);
            }
            Ok(render_target)
        })?;

        Ok(SwapChain {
            _dxgi_swap_chain: dxgi_swap_chain,
            _render_targets: render_targets,
            rtv_heap,
            dsv_heap,
        })
    }

    fn get_render_target_view(&self, index: usize) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        self.rtv_heap.get_cpu_descriptor_handle(index)
    }

    fn get_depth_stencil_view(&self, index: usize) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        self.dsv_heap.get_cpu_descriptor_handle(index)
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

    let mut swap_chain: Option<IDXGISwapChain1> = None;
    let swap_chain = unsafe {
        factory.CreateSwapChainForHwnd(
            command_queue,
            hwnd,
            &desc,
            std::ptr::null(),
            None,
            &mut swap_chain,
        )
    }
    .and_some(swap_chain)?
    .cast::<IDXGISwapChain3>()?;

    unsafe { factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER) }.ok()?;

    Ok(swap_chain)
}

impl Frames {
    fn new(device: &ID3D12Device, swap_chain: &SwapChain) -> Result<Frames> {
        let frames = try_array_init(|i| -> Result<Frame> {
            Ok(Frame {
                command_allocators: Default::default(),
                next_command_allocator: Default::default(),
                fence_value: Default::default(),
                render_data: Arc::new(RenderData::new(
                    device,
                    swap_chain.get_render_target_view(i),
                    D3D12_CPU_DESCRIPTOR_HANDLE { ptr: 0 },
                )?),
            })
        })?;

        let device = device.cast()?;

        Ok(Frames {
            device,
            current_index: 0,
            frames,
            idle_command_lists: Default::default(),
            command_lists: Default::default(),
        })
    }

    fn start_frame(&mut self, command_queue: &SynchronizedCommandQueue) -> Result<Arc<RenderData>> {
        let frame = &mut self.frames[self.current_index];
        frame.start(command_queue)?;
        Ok(frame.render_data.clone())
    }

    fn end_frame(&mut self, command_queue: &mut SynchronizedCommandQueue) -> Result<()> {
        command_queue.execute_command_lists(&self.command_lists);

        let frame = &mut self.frames[self.current_index];
        frame.end(command_queue)?;

        self.current_index = (self.current_index + 1) % FRAME_COUNT;
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

        unsafe {
            command_list
                .Reset(frame.get_command_allocator(&self.device)?, None)
                .ok()?
        }

        Ok(command_list)
    }
}

impl Frame {
    fn start(&mut self, command_queue: &SynchronizedCommandQueue) -> Result<()> {
        command_queue.wait_for_gpu(self.fence_value)?;
        for ca in &self.command_allocators {
            unsafe { ca.Reset().ok()? }
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

impl RenderData {
    fn new(
        device: &ID3D12Device,
        render_target_view: D3D12_CPU_DESCRIPTOR_HANDLE,
        shadow_depth_view: D3D12_CPU_DESCRIPTOR_HANDLE,
    ) -> Result<RenderData> {
        Ok(RenderData {
            render_target_view,
            shadow_depth_view,
        })
    }
}
