use array_init::try_array_init;
use bindings::Windows::Win32::{
    Foundation::HWND,
    Graphics::{Direct3D12::*, Dxgi::*},
};
use d3dx12::*;
use dxsample::*;
use futures::{*, executor::*, future::*, task::SpawnExt};
use windows::*;

use crate::State;

const FRAME_COUNT: usize = 2;
const MAX_CONCURRENT_TASKS: usize = 6;

pub struct Renderer {
    device: ID3D12Device,
    command_queue: SynchronizedCommandQueue,
    swap_chain: SwapChain,
    thread_pool: ThreadPool,
    frames: Frames,
}

unsafe impl Send for Renderer{}
unsafe impl Sync for Renderer{}

pub struct SwapChain {
    dxgi_swap_chain: IDXGISwapChain3,
    render_targets: [ID3D12Resource; FRAME_COUNT],
    rtv_heap: RtvDescriptorHeap,
}

pub struct Frames {
    device: ID3D12Device4, // TODO: do we need the one in Renderer as well?
    current_index: usize,
    data: [Frame; FRAME_COUNT],
    idle_command_lists: Vec<ID3D12GraphicsCommandList>,
    command_lists: Vec<ID3D12GraphicsCommandList>,
}

#[derive(Default)]
pub struct Frame {
    command_allocators: Vec<ID3D12CommandAllocator>,
    next_command_allocator: usize,
    fence_value: u64,
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

        let mut thread_pool_builder = ThreadPoolBuilder::new();
        thread_pool_builder.name_prefix("renderer-");
        let thread_pool = thread_pool_builder.create().unwrap();

        let frames = Frames::new(&device)?;

        Ok(Renderer {
            device,
            command_queue,
            swap_chain,
            thread_pool,
            frames,
        })
    }

    pub fn render(&mut self, state: &State) -> Result<()> {
        let frame = self.frames.start_frame(&self.command_queue)?;

        let cl = self.frames.get_command_list()?;
        let pre_render = self.pre_render(cl);

        //let cl = self.frames.get_command_list()?;
        //let post_render = self.post_render(cl);

        //let render_futures = join(pre_render, post_render);        

        let x = self.thread_pool.spawn_obj_ok(pre_render);


        self.frames.end_frame(&mut self.command_queue)?;
        Ok(())
    }

    async fn pre_render(&self, cl: ID3D12GraphicsCommandList) -> Result<ID3D12GraphicsCommandList> {
        unsafe { cl.Close() }.ok()?;
        Ok(cl)
    }

    async fn post_render(&self, cl: ID3D12GraphicsCommandList) -> Result<ID3D12GraphicsCommandList> {
        unsafe { cl.Close() }.ok()?;
        Ok(cl)
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

        let render_targets = try_array_init(|i| -> Result<ID3D12Resource> {
            let render_target: ID3D12Resource = unsafe { dxgi_swap_chain.GetBuffer(i as u32) }?;
            unsafe {
                rtv_heap.create_render_target_view(device, &render_target, None, i);
            }
            Ok(render_target)
        })?;

        Ok(SwapChain {
            dxgi_swap_chain,
            render_targets,
            rtv_heap,
        })
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
    fn new(device: &ID3D12Device) -> Result<Frames> {
        let data = try_array_init(|i| -> Result<Frame> { Ok(Default::default()) })?;
        let device = device.cast()?;

        Ok(Frames {
            device,
            current_index: 0,
            data,
            idle_command_lists: Default::default(),
            command_lists: Default::default(),
        })
    }

    fn start_frame(&mut self, command_queue: &SynchronizedCommandQueue) -> Result<&mut Frame> {
        let frame = &mut self.data[self.current_index];
        frame.start(command_queue)?;
        Ok(frame)
    }

    fn end_frame(&mut self, command_queue: &mut SynchronizedCommandQueue) -> Result<()> {
        command_queue.execute_command_lists(&self.command_lists);

        let frame = &mut self.data[self.current_index];
        frame.end(command_queue);

        self.current_index = (self.current_index + 1) % FRAME_COUNT;
        self.idle_command_lists.append(&mut self.command_lists);

        Ok(())
    }

    fn get_command_list(&mut self) -> Result<ID3D12GraphicsCommandList> {
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

        let frame = &mut self.data[self.current_index];

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
