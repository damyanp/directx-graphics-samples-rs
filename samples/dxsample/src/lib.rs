use std::mem::transmute;
use windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY;
use windows::runtime::*;
use windows::Win32::{
    Foundation::*,
    Graphics::{Direct3D11::*, Direct3D12::*, Dxgi::*},
    System::LibraryLoader::*,
    System::Threading::*,
    System::WindowsProgramming::*,
    UI::WindowsAndMessaging::*,
};

pub trait DXSample {
    fn new(command_line: &SampleCommandLine) -> Result<Self>
    where
        Self: Sized;

    fn bind_to_window(&mut self, hwnd: &HWND) -> Result<()>;

    fn update(&mut self) {}
    fn render(&mut self) {}
    fn on_key_up(&mut self, _key: VIRTUAL_KEY) {}
    fn on_key_down(&mut self, _key: VIRTUAL_KEY) {}

    fn title(&self) -> String {
        "D3D12 Hello Triangle".into()
    }

    fn window_size(&self) -> (i32, i32) {
        (640, 480)
    }
}

#[derive(Clone, Default)]
pub struct SampleCommandLine {
    pub use_warp_device: bool,
}

pub fn build_command_line() -> SampleCommandLine {
    let mut use_warp_device = false;

    for arg in std::env::args() {
        if arg.eq_ignore_ascii_case("-warp") || arg.eq_ignore_ascii_case("/warp") {
            use_warp_device = true;
        }
    }

    SampleCommandLine { use_warp_device }
}

pub fn run_sample<S>() -> Result<()>
where
    S: DXSample,
{
    let instance = unsafe { GetModuleHandleA(None) };
    debug_assert_ne!(instance.0, 0);

    let wc = WNDCLASSEXA {
        cbSize: std::mem::size_of::<WNDCLASSEXA>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc::<S>),
        hInstance: instance,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) },
        lpszClassName: PSTR(b"RustWindowClass\0".as_ptr() as _),
        ..Default::default()
    };

    let command_line = build_command_line();
    let mut sample = S::new(&command_line)?;

    let size = sample.window_size();

    let atom = unsafe { RegisterClassExA(&wc) };
    debug_assert_ne!(atom, 0);

    let mut window_rect = RECT {
        left: 0,
        top: 0,
        right: size.0,
        bottom: size.1,
    };
    unsafe { AdjustWindowRect(&mut window_rect, WS_OVERLAPPEDWINDOW, false) };

    let mut title = sample.title();

    if command_line.use_warp_device {
        title.push_str(" (WARP)");
    }

    let hwnd = unsafe {
        CreateWindowExA(
            Default::default(),
            "RustWindowClass",
            title,
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            window_rect.right - window_rect.left,
            window_rect.bottom - window_rect.top,
            None, // no parent window
            None, // no menus
            instance,
            &mut sample as *mut _ as _,
        )
    };
    debug_assert_ne!(hwnd.0, 0);

    sample.bind_to_window(&hwnd)?;

    unsafe { ShowWindow(hwnd, SW_SHOW) };

    loop {
        let mut message = MSG::default();

        if unsafe { PeekMessageA(&mut message, None, 0, 0, PM_REMOVE) }.into() {
            unsafe {
                TranslateMessage(&message);
                DispatchMessageA(&message);
            }

            if message.message == WM_QUIT {
                break;
            }
        }
    }

    Ok(())
}

fn sample_wndproc<S: DXSample>(sample: &mut S, message: u32, wparam: WPARAM) -> bool {
    match message {
        WM_KEYDOWN => {
            sample.on_key_down(VIRTUAL_KEY(wparam.0 as u16));
            true
        }
        WM_KEYUP => {
            sample.on_key_up(VIRTUAL_KEY(wparam.0 as u16));
            true
        }
        WM_PAINT => {
            sample.update();
            sample.render();
            true
        }
        _ => false,
    }
}

#[allow(non_snake_case)]
#[cfg(target_pointer_width = "32")]
unsafe fn SetWindowLong(window: HWND, index: WINDOW_LONG_PTR_INDEX, value: isize) -> isize {
    SetWindowLongA(window, index, value as _) as _
}

#[allow(non_snake_case)]
#[cfg(target_pointer_width = "64")]
unsafe fn SetWindowLong(window: HWND, index: WINDOW_LONG_PTR_INDEX, value: isize) -> isize {
    SetWindowLongPtrA(window, index, value)
}

#[allow(non_snake_case)]
#[cfg(target_pointer_width = "32")]
unsafe fn GetWindowLong(window: HWND, index: WINDOW_LONG_PTR_INDEX) -> isize {
    GetWindowLongA(window, index) as _
}

#[allow(non_snake_case)]
#[cfg(target_pointer_width = "64")]
unsafe fn GetWindowLong(window: HWND, index: WINDOW_LONG_PTR_INDEX) -> isize {
    GetWindowLongPtrA(window, index)
}

extern "system" fn wndproc<S: DXSample>(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_CREATE => {
            unsafe {
                let create_struct: &CREATESTRUCTA = transmute(lparam);
                SetWindowLong(window, GWLP_USERDATA, create_struct.lpCreateParams as _);
            }
            LRESULT::default()
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT::default()
        }
        _ => {
            let user_data = unsafe { GetWindowLong(window, GWLP_USERDATA) };
            let sample = std::ptr::NonNull::<S>::new(user_data as _);
            let handled = sample.map_or(false, |mut s| {
                sample_wndproc(unsafe { s.as_mut() }, message, wparam)
            });

            if handled {
                LRESULT::default()
            } else {
                unsafe { DefWindowProcA(window, message, wparam, lparam) }
            }
        }
    }
}

fn get_hardware_adapter(factory: &IDXGIFactory4) -> Result<IDXGIAdapter1> {
    for i in 0.. {
        let adapter = unsafe { factory.EnumAdapters1(i) }?;
        let desc = unsafe { adapter.GetDesc1() }?;

        if (DXGI_ADAPTER_FLAG::from(desc.Flags) & DXGI_ADAPTER_FLAG_SOFTWARE)
            != DXGI_ADAPTER_FLAG_NONE
        {
            // Don't select the Basic Render Driver adapter. If you want a
            // software adapter, pass in "/warp" on the command line.
            continue;
        }

        // We need the variant where we pass in NULL for the outparam.
        #[link(name = "d3d12")]
        extern "system" {
            pub fn D3D12CreateDevice(
                padapter: RawPtr,
                minimumfeaturelevel: D3D_FEATURE_LEVEL,
                riid: *const GUID,
                ppdevice: *mut *mut ::std::ffi::c_void,
            ) -> HRESULT;
        }

        // Check to see whether the adapter supports Direct3D 12, but don't
        // create the actual device yet.
        if unsafe {
            D3D12CreateDevice(
                std::mem::transmute_copy(&adapter),
                D3D_FEATURE_LEVEL_11_0,
                &ID3D12Device::IID,
                std::ptr::null_mut(),
            )
        }
        .is_ok()
        {
            return Ok(adapter);
        }
    }

    unreachable!()
}

pub fn create_device(command_line: &SampleCommandLine) -> Result<(IDXGIFactory4, ID3D12Device)> {
    if cfg!(debug_assertions) {
        unsafe {
            let mut debug: Option<ID3D12Debug> = None;
            if let Some(debug) = D3D12GetDebugInterface(&mut debug).ok().and_then(|_| debug) {
                debug.EnableDebugLayer();
            }
        }
    }

    let dxgi_factory_flags = if cfg!(debug_assertions) {
        DXGI_CREATE_FACTORY_DEBUG
    } else {
        0
    };

    let dxgi_factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(dxgi_factory_flags) }?;

    let adapter = if command_line.use_warp_device {
        unsafe { dxgi_factory.EnumWarpAdapter() }
    } else {
        get_hardware_adapter(&dxgi_factory)
    }?;

    let mut device: Option<ID3D12Device> = None;
    unsafe { D3D12CreateDevice(adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }?;

    Ok((dxgi_factory, device.unwrap()))
}

/// A command queue, a fence, and an event.  This allows us to synchronize the
/// GPU or CPU. with each other.
pub struct SynchronizedCommandQueue {
    pub queue: ID3D12CommandQueue,
    pub fence: ID3D12Fence,
    fence_value: u64,
    fence_event: HANDLE,
}

impl SynchronizedCommandQueue {
    pub fn new(device: &ID3D12Device, queue_type: D3D12_COMMAND_LIST_TYPE) -> Result<Self> {
        let command_queue = unsafe {
            device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                Type: queue_type,
                ..Default::default()
            })
        }?;

        let fence = unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE) }?;
        let fence_event = unsafe { CreateEventA(std::ptr::null_mut(), false, false, None) };

        Ok(SynchronizedCommandQueue {
            queue: command_queue,
            fence,
            fence_value: 1,
            fence_event,
        })
    }

    #[allow(non_snake_case)]
    /// # Safety
    /// commandlists is expected to be an array of size numcommandlists.  Make
    /// sure it is!
    pub unsafe fn ExecuteCommandLists(
        &self,
        numcommandlists: u32,
        commandlists: *mut Option<ID3D12CommandList>,
    ) {
        self.queue
            .ExecuteCommandLists(numcommandlists, commandlists)
    }

    pub fn execute_command_lists(&self, command_lists: &[ID3D12GraphicsCommandList]) {
        unsafe {
            self.ExecuteCommandLists(
                command_lists.len() as u32,
                command_lists.as_ptr() as *mut Option<ID3D12CommandList>,
            );
        }
    }

    pub fn enqueue_signal(&mut self) -> Result<u64> {
        unsafe { self.queue.Signal(&self.fence, self.fence_value) }?;

        let signaled_value = self.fence_value;
        self.fence_value += 1;

        Ok(signaled_value)
    }

    pub fn wait_for_gpu(&self, signaled_value: u64) -> Result<()> {
        unsafe {
            self.fence
                .SetEventOnCompletion(signaled_value, self.fence_event)?;
            WaitForSingleObject(self.fence_event, INFINITE);
        }

        Ok(())
    }

    pub fn signal_and_wait_for_gpu(&mut self) -> Result<()> {
        let enqueued_signal = self.enqueue_signal()?;
        self.wait_for_gpu(enqueued_signal)?;
        Ok(())
    }
}
