//! Where Flat's frames show: each frame's pixels written into a texture of
//! its own on the host's device, which the host samples while it holds it.
use kinogaki_render_api::display::{Display, Shown, wgpu};
use kinogaki_render_api::{ViewId, ViewPass};
use std::sync::Arc;

/// The host's device, and how many textures were made (each one's key).
pub(crate) struct Textures {
    device: wgpu::Device,
    queue: wgpu::Queue,
    made: u64,
}

impl Textures {
    pub(crate) fn new(device: wgpu::Device, queue: wgpu::Queue) -> Textures {
        Textures {
            device,
            queue,
            made: 0,
        }
    }
}

impl Display for Textures {
    /// A new texture holding `frame`'s pixels: nothing writes it again, so
    /// the host may sample it for as long as it holds it.
    fn acquire(&mut self, frame: &Arc<ViewPass>) -> Option<Shown> {
        let [width, height] = frame.size();
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("flat frame"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let bytes = frame.image.plane(0)?;
        self.queue.write_texture(
            texture.as_image_copy(),
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 8),
                rows_per_image: Some(height),
            },
            size,
        );
        self.made += 1;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Some(Shown::new(frame.clone(), (self.made, 0), view, texture))
    }

    /// It keeps nothing a view.
    fn drop_view(&mut self, _view: ViewId) {}
}
