use bindings::Windows::Win32::{
    Foundation::*,
    Graphics::{Direct3D12::*, Dxgi::*},
};
use d3dx12::*;
use dxsample::*;
use windows::*;

mod d3d12_hello_window {
    use std::convert::TryInto;

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
        command_allocator: ID3D12CommandAllocator,
        command_list: ID3D12GraphicsCommandList,
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
                BufferCount: FRAME_COUNT as u32,
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

            let command_allocator = unsafe {
                self.device
                    .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
            }?;

            let command_list: ID3D12GraphicsCommandList = unsafe {
                self.device.CreateCommandList(
                    0,
                    D3D12_COMMAND_LIST_TYPE_DIRECT,
                    &command_allocator,
                    None,
                )
            }?;
            unsafe { command_list.Close() }.ok()?;

            self.resources = Some(Resources {
                command_queue,
                swap_chain,
                frame_index,
                render_targets,
                rtv_heap,
                command_allocator,
                command_list,
            });

            Ok(())
        }

        fn title(&self) -> String {
            "D3D12 Hello Window".into()
        }

        fn window_size(&self) -> (i32, i32) {
            (1280, 720)
        }

        fn render(&mut self) {
            let resources = match &mut self.resources {
                Some(it) => it,
                _ => return,
            };
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
        unsafe { command_list.Reset(&resources.command_allocator, None) }.ok()?;

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

    fn wait_for_previous_frame(resources: &mut Resources) {
        // WAITING FOR THE FRAME TO COMPLETE BEFORE CONTINUING IS NOT BEST
        // PRACTICE. This is code implemented as such for simplicity. The
        // D3D12HelloFrameBuffering sample illustrates how to use fences for
        // efficient resource usage and to maximize GPU utilization.

        resources.command_queue.signal_and_wait_for_gpu().unwrap();

        resources.frame_index = unsafe {
            resources
                .swap_chain
                .GetCurrentBackBufferIndex()
                .try_into()
                .unwrap()
        };
    }
}

fn main() -> Result<()> {
    run_sample::<d3d12_hello_window::Sample>()?;

    Ok(())
}
