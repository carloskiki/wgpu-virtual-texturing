use crate::setup::WgpuContext;

pub struct Textures {
    pub prepass_texture: wgpu::Texture,
    pub prepass_depth_texture: wgpu::Texture,
    pub page_table_texture: wgpu::Texture,
    pub physical_texture: wgpu::Texture,
}

impl Textures {
    pub fn new(context: &WgpuContext, virtual_texture_page_wide: u32) -> Self {
        let prepass_texture_size = wgpu::Extent3d {
            width: context.window_size.width / 10,
            height: context.window_size.height / 10,
            depth_or_array_layers: 1,
        };
        let prepass_texture = context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("prepass texture"),
            size: prepass_texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Uint,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let prepass_depth_texture = context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("prepass depth texture"),
            size: prepass_texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let max_side_len = context.device.limits().max_texture_dimension_2d;
        assert!(virtual_texture_page_wide.is_power_of_two());
        debug_assert!(virtual_texture_page_wide <= max_side_len);
        let page_table_texture =
            context.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Page table texture"),
                size: wgpu::Extent3d {
                    width: virtual_texture_page_wide,
                    height: virtual_texture_page_wide,
                    depth_or_array_layers: 1,
                },
                mip_level_count: f32::log2(virtual_texture_page_wide as f32) as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
        let physical_texture = context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Physical texture"),
            size: wgpu::Extent3d {
                width: max_side_len,
                height: max_side_len,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: context.surface_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        Self {
            prepass_texture,
            prepass_depth_texture,
            page_table_texture,
            physical_texture,
        }
    }
}
