use std::ffi::{c_char, c_void, CString};
use std::sync::OnceLock;

pub const GL_FRAMEBUFFER_BINDING: u32 = 0x8CA6;

type EglGetProcAddressFn = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type GlxGetProcAddressFn = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type GlGetIntegervFn = unsafe extern "C" fn(u32, *mut i32);

static EGL_GET_PROC: OnceLock<Option<EglGetProcAddressFn>> = OnceLock::new();
static GLX_GET_PROC: OnceLock<Option<GlxGetProcAddressFn>> = OnceLock::new();
static GL_GET_INTEGERV: OnceLock<Option<GlGetIntegervFn>> = OnceLock::new();

pub unsafe fn get_proc_address(name: &str) -> *mut c_void {
    let c_name = match CString::new(name) {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    // 1. Try eglGetProcAddress (Wayland & EGL default)
    let egl = *EGL_GET_PROC.get_or_init(|| {
        let handle = libc::dlopen(c"libEGL.so.1".as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL);
        if !handle.is_null() {
            let sym = libc::dlsym(handle, c"eglGetProcAddress".as_ptr());
            if !sym.is_null() {
                return Some(std::mem::transmute(sym));
            }
        }
        None
    });
    if let Some(f) = egl {
        let ptr = f(c_name.as_ptr());
        if !ptr.is_null() {
            return ptr;
        }
    }

    // 2. Try glXGetProcAddressARB (X11 / GLX default)
    let glx = *GLX_GET_PROC.get_or_init(|| {
        let handle = libc::dlopen(c"libGLX.so.0".as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL);
        if !handle.is_null() {
            let sym = libc::dlsym(handle, c"glXGetProcAddressARB".as_ptr());
            if !sym.is_null() {
                return Some(std::mem::transmute(sym));
            }
        }
        None
    });
    if let Some(f) = glx {
        let ptr = f(c_name.as_ptr());
        if !ptr.is_null() {
            return ptr;
        }
    }

    // 3. Fallback to dlsym(RTLD_DEFAULT)
    libc::dlsym(libc::RTLD_DEFAULT, c_name.as_ptr())
}

pub unsafe fn query_bound_framebuffer() -> i32 {
    let get_integerv = *GL_GET_INTEGERV.get_or_init(|| {
        let ptr = get_proc_address("glGetIntegerv");
        if !ptr.is_null() {
            Some(std::mem::transmute(ptr))
        } else {
            None
        }
    });

    let mut fbo: i32 = 0;
    if let Some(f) = get_integerv {
        f(GL_FRAMEBUFFER_BINDING, &mut fbo);
    }
    fbo
}
