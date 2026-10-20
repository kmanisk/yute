use crate::gl_loader;
use crate::player_embedded::EmbeddedPlayer;
use crate::render_context::MpvRenderContext;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::GLArea;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

#[allow(dead_code)]
pub struct VideoArea {
    pub widget: GLArea,
    render_context: Rc<RefCell<Option<MpvRenderContext>>>,
}

impl VideoArea {
    pub fn new(player: Arc<EmbeddedPlayer>) -> Self {
        let gl_area = GLArea::new();
        gl_area.set_has_depth_buffer(false);
        gl_area.set_has_stencil_buffer(false);
        gl_area.set_hexpand(true);
        gl_area.set_vexpand(true);
        gl_area.set_auto_render(false);

        let render_context = Rc::new(RefCell::new(None));
        let rc_realize = Rc::clone(&render_context);
        let rc_render = Rc::clone(&render_context);
        let rc_unrealize = Rc::clone(&render_context);

        let player_realize = Arc::clone(&player);

        gl_area.connect_realize(move |area| {
            area.make_current();
            let ctx_result = unsafe { MpvRenderContext::create(player_realize.mpv.ctx.as_ptr()) };

            match ctx_result {
                Ok(mut ctx) => {
                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
                    let weak_area = area.downgrade();

                    glib::MainContext::default().spawn_local(async move {
                        while let Some(()) = rx.recv().await {
                            if let Some(a) = weak_area.upgrade() {
                                a.queue_render();
                            }
                        }
                    });

                    unsafe {
                        ctx.set_update_callback(move || {
                            let _ = tx.send(());
                        });
                    }

                    *rc_realize.borrow_mut() = Some(ctx);
                    tracing::info!("MPV render context created and attached to GtkGLArea");
                }
                Err(err) => {
                    tracing::error!("Failed to create MPV render context: {err}");
                }
            }
        });

        gl_area.connect_render(move |area, _| {
            area.make_current();
            let fbo = unsafe { gl_loader::query_bound_framebuffer() };
            let scale = area.scale_factor();
            let width = (area.width() * scale).max(1);
            let height = (area.height() * scale).max(1);

            if let Some(ref ctx) = *rc_render.borrow() {
                unsafe {
                    ctx.render(fbo, width, height);
                    ctx.report_swap();
                }
            }
            glib::Propagation::Stop
        });

        gl_area.connect_unrealize(move |area| {
            area.make_current();
            // Drop render context safely before GL context destruction
            *rc_unrealize.borrow_mut() = None;
            tracing::info!("MPV render context freed on GLArea unrealize");
        });

        Self {
            widget: gl_area,
            render_context,
        }
    }
}
