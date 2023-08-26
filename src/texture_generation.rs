/// The dimensions of a texture to add to the Virtual Texture.
#[derive(Copy, Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextureDims {
    extent: wgpu::Extent3d,
}

/// Ordered by `height`, then `width` if `height` is equal.
impl PartialOrd for TextureDims {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let mut output = self.extent.height.cmp(&other.extent.height);
        if output == std::cmp::Ordering::Equal {
            output = self.extent.width.cmp(&other.extent.width);
        };
        Some(output)
    }
}

impl Ord for TextureDims {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

/// Offset of a subtexture on a Virtual Texture.
pub type UvOffset = (u32, u32);

/// This function creates a Virtual Texture from the given Textures.
pub fn create_virt_texture(textures: &[TextureDims]) -> Vec<UvOffset> {
    /// stores the boundaries of a bottom line section, and if it is bounded or not.
    struct BottomSection {
        begin: u32,
        end: u32,
        height: u32,
        bounded_at: u32,
    }
    // Smallest power of 2 bigger than min_area side_length.
    let virtual_texture_width = {
        let min_area = textures
            .iter()
            .map(|dims| dims.extent.width * dims.extent.height)
            .sum::<u32>();
        let mut n = (min_area as f64).sqrt().ceil() as u32;
        n -= 1;
        n |= n >> 1;
        n |= n >> 2;
        n |= n >> 4;
        n |= n >> 8;
        n |= n >> 16;
        n + 1
    };
    // Bottom line is initially like this:
    //      |                     |
    //      |                     |
    // (0,0)|_____________________|(virtual_texture_width, 0)
    let mut bottom_line: Vec<BottomSection> = vec![BottomSection {
        begin: 0,
        end: virtual_texture_width,
        height: 0,
        bounded_at: virtual_texture_width,
    }];

    textures
        .iter()
        .map(|dims| -> UvOffset {
            // The index of the best choice in case we can't fit the rect fully without hanging
            let mut least_hanging: usize = 0;
            let mut to_insert: Option<BottomSection, usize> = None;

            for (idx, section) in bottom_line.iter_mut().enumerate() {
                let len = section.end - section.begin;
                if len > dims.extent.width {
                    to_insert = Some(BottomSection {
                        begin: section.begin,
                        end: section.begin + dims.extent.width,
                        height: section.height + dims.extent.height,
                        bounded: false,
                    })
                }
            }
        })
        .collect()
}
