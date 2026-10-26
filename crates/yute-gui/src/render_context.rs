use crate::gl_loader;
use libmpv2_sys::*;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::ptr;

pub struct MpvRenderContext {
    ctx: *mut mpv_render_context,
}

unsafe impl Send for MpvRenderContext {}
unsafe impl Sync for MpvRenderContext {}

unsafe extern "C" fn get_proc_address_cb(_ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    if name.is_null() {
        return ptr::null_mut();
    }
    let name_str = CStr::from_ptr(name).to_string_lossy();
    gl_loader::get_proc_address(&name_str)
}

impl MpvRenderContext {
    pub unsafe fn create(mpv: *mut mpv_handle) -> Result<Self, i32> {
        let mut gl_params = mpv_opengl_init_params {
            get_proc_address: Some(get_proc_address_cb),
            get_proc_address_ctx: ptr::null_mut(),
        };

        let mut params = [
            mpv_render_param {
                type_: mpv_render_param_type_MPV_RENDER_PARAM_API_TYPE,
                data: MPV_RENDER_API_TYPE_OPENGL.as_ptr() as *mut c_void,
            },
            mpv_render_param {
                type_: mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                data: &mut gl_params as *mut _ as *mut c_void,
            },
            mpv_render_param {
                type_: mpv_render_param_type_MPV_RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            },
        ];

        let mut ctx: *mut mpv_render_context = ptr::null_mut();
        let err = mpv_render_context_create(&mut ctx, mpv, params.as_mut_ptr());
        if err < 0 {
            return Err(err);
        }

        Ok(Self { ctx })
    }

    pub unsafe fn set_update_callback<F: Fn() + Send + 'static>(&mut self, cb: F) {
        unsafe extern "C" fn callback_wrapper<F: Fn()>(ctx: *mut c_void) {
            if !ctx.is_null() {
                let closure = &*(ctx as *mut F);
                closure();
            }
        }
        let boxed = Box::into_raw(Box::new(cb));
        mpv_render_context_set_update_callback(
            self.ctx,
            Some(callback_wrapper::<F>),
            boxed as *mut c_void,
        );
    }

    pub unsafe fn update(&self) -> u64 {
        mpv_render_context_update(self.ctx)
    }

    pub unsafe fn render(&self, fbo: i32, width: i32, height: i32) {
        let mut fbo_data = mpv_opengl_fbo {
            fbo,
            w: width,
            h: height,
            internal_format: 0,
        };
        let mut flip_y: c_int = 1;

        let mut params = [
            mpv_render_param {
                type_: mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_FBO,
                data: &mut fbo_data as *mut _ as *mut c_void,
            },
            mpv_render_param {
                type_: mpv_render_param_type_MPV_RENDER_PARAM_FLIP_Y,
                data: &mut flip_y as *mut _ as *mut c_void,
            },
            mpv_render_param {
                type_: mpv_render_param_type_MPV_RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            },
        ];

        mpv_render_context_render(self.ctx, params.as_mut_ptr());
    }

    pub unsafe fn report_swap(&self) {
        mpv_render_context_report_swap(self.ctx);
    }
}

impl Drop for MpvRenderContext {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe {
                mpv_render_context_set_update_callback(self.ctx, None, ptr::null_mut());
                mpv_render_context_free(self.ctx);
            }
            self.ctx = ptr::null_mut();
        }
    }
}
