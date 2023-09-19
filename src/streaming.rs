use std::sync::{mpsc::Sender, Arc};

use crate::{setup::WgpuContext, textures::Textures, storage::TextureStorage};

const PREPASS_BYTES_PER_TEXEL: usize = 4;

pub struct StreamingHandle {
    texture_storage: TextureStorage,
    prepass_read_buffer: Arc<wgpu::Buffer>,
    sender: Sender<()>,
}

impl StreamingHandle {
    pub fn new(
        context: Arc<WgpuContext>,
        textures: Arc<Textures>,
        storage: TextureStorage,
    ) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let prepass_read_buffer = Arc::new(context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("prepass_read_buffer"),
            size: (textures.prepass_texture.width()
                * textures.prepass_texture.height()
                * PREPASS_BYTES_PER_TEXEL as u32) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        }));
        let move_buffer = Arc::clone(&prepass_read_buffer);
        std::thread::spawn(move || loop {
            rx.recv().unwrap();
            let buffer_view = move_buffer.slice(..).get_mapped_range();
            move_buffer.unmap();
            let mut required_pages = buffer_view
                .chunks_exact(PREPASS_BYTES_PER_TEXEL as usize)
                .map(PageId::from_bytes)
                .collect::<Vec<_>>();

            required_pages.sort_unstable_by(|a, b| a.cmp(b).reverse());
            required_pages.dedup();

            // Group by same shard, then ...
            // Stream in the textures
            // Create page_table from highest mip level to lowest
            // Write to texture only the modified bit, but
        });

        Self {
            sender: tx,
            prepass_read_buffer,
            texture_storage: storage,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageId {
    page_x: u16,
    page_y: u16,
    mip_level: u8,
}

impl PageId {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        debug_assert!(bytes.len() == 4);

        let page_x_high = bytes[0];
        let page_x_low = bytes[1] >> 2;
        let page_y_high = bytes[1] & 0b0000_0011;
        let page_y_mid = bytes[2];
        let page_y_low = bytes[3] >> 4;
        let mip_level = bytes[3] & 0b0000_1111;

        let page_x = page_x_low as u16 | (page_x_high as u16) << 8;
        let page_y = page_y_low as u16 | (page_y_mid as u16) << 2 | (page_y_high as u16) << 8;
        Self {
            page_x,
            page_y,
            mip_level,
        }
    }
}

impl PartialOrd for PageId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// PageIds are sorted by mip level, then by y, then by x.
impl Ord for PageId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.mip_level
            .cmp(&other.mip_level)
            .then(self.page_y.cmp(&other.page_y))
            .then(self.page_x.cmp(&other.page_x))
    }
}
