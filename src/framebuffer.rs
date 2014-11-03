use std::kinds::marker::ContravariantLifetime;
use std::ptr;
use std::sync::Arc;

use texture::{mod, Texture};
use uniforms::Uniforms;
use {DisplayImpl, VertexBuffer, IndexBuffer, Program, DrawParameters, Surface};
use {FrameBufferObject};

use {vertex_buffer, index_buffer, program};
use {gl, context, libc};

/// Creates a framebuffer that allows you to draw on something.
pub struct FrameBuffer<'a> {
    attachments: FramebufferAttachments,
    marker: ContravariantLifetime<'a>,
}

impl<'a> FrameBuffer<'a> {
    pub fn new<'a>() -> FrameBuffer<'a> {
        FrameBuffer {
            attachments: FramebufferAttachments {
                colors: Vec::new(),
                depth: None,
                stencil: None
            },
            marker: ContravariantLifetime
        }
    }

    pub fn with_texture<T: 'a>(mut self, texture: &'a mut T) -> FrameBuffer<'a> where T: Texture {
        self.attachments.colors.push(texture::get_id(texture.get_implementation()));
        self
    }
}

#[deriving(Hash, Clone, PartialEq, Eq)]
pub struct FramebufferAttachments {
    colors: Vec<gl::types::GLuint>,
    depth: Option<gl::types::GLuint>,
    stencil: Option<gl::types::GLuint>,
}

/// Draws everything.
pub fn draw<V, U: Uniforms>(display: &Arc<DisplayImpl>,
    framebuffer: Option<&FramebufferAttachments>, vertex_buffer: &VertexBuffer<V>,
    index_buffer: &IndexBuffer, program: &Program, uniforms: &U, draw_parameters: &DrawParameters)
{
    let fbo_id = get_framebuffer(display, framebuffer);

    let (vb_id, vb_elementssize, vb_bindingsclone) = vertex_buffer::get_clone(vertex_buffer);
    let (ib_id, ib_elemcounts, ib_datatype, ib_primitives) =
        index_buffer::get_clone(index_buffer);
    let program_id = program::get_program_id(program);
    let uniforms = uniforms.to_binder();
    let uniforms_locations = program::get_uniforms_locations(program);
    let draw_parameters = draw_parameters.clone();

    let (tx, rx) = channel();

    display.context.exec(proc(gl, state, version, _) {
        unsafe {
            if state.draw_framebuffer != fbo_id {
                if version >= &context::GlVersion(3, 0) {
                    gl.BindFramebuffer(gl::DRAW_FRAMEBUFFER, fbo_id.unwrap_or(0));
                    state.draw_framebuffer = fbo_id.clone();
                } else {
                    gl.BindFramebufferEXT(gl::FRAMEBUFFER_EXT, fbo_id.unwrap_or(0));
                    state.draw_framebuffer = fbo_id.clone();
                    state.read_framebuffer = fbo_id.clone();
                }
            }

            // binding program
            if state.program != program_id {
                gl.UseProgram(program_id);
                state.program = program_id;
            }

            // binding program uniforms
            uniforms.0(gl, |name| {
                uniforms_locations
                    .find_equiv(name)
                    .map(|val| val.0)
            });

            // binding vertex buffer
            if state.array_buffer_binding != Some(vb_id) {
                gl.BindBuffer(gl::ARRAY_BUFFER, vb_id);
                state.array_buffer_binding = Some(vb_id);
            }

            // binding index buffer
            if state.element_array_buffer_binding != Some(ib_id) {
                gl.BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ib_id);
                state.element_array_buffer_binding = Some(ib_id);
            }

            // binding vertex buffer
            let mut locations = Vec::new();
            for &(ref name, vertex_buffer::VertexAttrib { offset, data_type, elements_count })
                in vb_bindingsclone.iter()
            {
                let loc = gl.GetAttribLocation(program_id, name.to_c_str().unwrap());
                locations.push(loc);

                if loc != -1 {
                    match data_type {
                        gl::BYTE | gl::UNSIGNED_BYTE | gl::SHORT | gl::UNSIGNED_SHORT |
                        gl::INT | gl::UNSIGNED_INT =>
                            gl.VertexAttribIPointer(loc as u32,
                                elements_count as gl::types::GLint, data_type,
                                vb_elementssize as i32, offset as *const libc::c_void),

                        _ => gl.VertexAttribPointer(loc as u32,
                                elements_count as gl::types::GLint, data_type, 0,
                                vb_elementssize as i32, offset as *const libc::c_void)
                    }
                    
                    gl.EnableVertexAttribArray(loc as u32);
                }
            }

            // sync-ing parameters
            draw_parameters.sync(gl, state);
            
            // drawing
            gl.DrawElements(ib_primitives, ib_elemcounts as i32, ib_datatype, ptr::null());

            // disable vertex attrib array
            for l in locations.iter() {
                gl.DisableVertexAttribArray(l.clone() as u32);
            }
        }

        tx.send(());
    });

    // synchronizing with the end of the draw
    // TODO: remove that after making sure that everything is ok
    rx.recv();
}

fn get_framebuffer(display: &Arc<DisplayImpl>, framebuffer: Option<&FramebufferAttachments>)
    -> Option<gl::types::GLuint>
{
    if let Some(framebuffer) = framebuffer {
        let mut framebuffers = display.framebuffer_objects.lock();

        if let Some(value) = framebuffers.find(framebuffer) {
            return Some(value.id);
        }

        let mut new_fbo = FrameBufferObject::new(display.clone());
        let new_fbo_id = new_fbo.id.clone();
        initialize_fbo(display, &mut new_fbo, framebuffer);
        framebuffers.insert(framebuffer.clone(), new_fbo);
        Some(new_fbo_id)

    } else {
        None
    }
}

fn initialize_fbo(display: &Arc<DisplayImpl>, fbo: &mut FrameBufferObject,
    content: &FramebufferAttachments)
{
    // TODO: missing implementation
}
