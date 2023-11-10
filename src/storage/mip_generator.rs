use crate::storage::{TextureStorage, TextureStorageError, PAGE_BORDER_SIZE, PAGE_SIZE};

pub struct MipLevelGen {
    next_mip: Option<Box<MipLevelGen>>,
    // (The row, the index of the row)
    stored_row: Option<(Box<[u8]>, usize)>,
    bytes_per_texel: u8,
    mip_level: u8,
    filter_mode: image::imageops::FilterType,
}

impl MipLevelGen {
    /// Creates a new generator
    pub fn from_mip(
        mip: u8,
        base_mip: u8,
        bytes_per_texel: u8,
        filter_mode: image::imageops::FilterType,
    ) -> Self {
        let next_mip = (mip > base_mip).then(|| {
            Box::new(Self::from_mip(
                mip,
                base_mip + 1,
                bytes_per_texel,
                filter_mode,
            ))
        });
        Self {
            stored_row: None,
            mip_level: base_mip,
            next_mip,
            bytes_per_texel,
            filter_mode,
        }
    }

    /// Writes a row to the generator.
    fn write_row(
        &mut self,
        row: Box<[u8]>,
        index: usize,
        storage: &mut TextureStorage,
    ) -> Result<(), TextureStorageError> {
        storage.write_row(self.mip_level, index as u16, &row)?;

        if self.stored_row.is_none() {
            assert!(index % 2 == 0);
            self.stored_row = Some((row, index));
        } else {
            let (stored_row, index) = self.stored_row.take().unwrap();
            self.mip_two_rows((&stored_row, &row), index, storage)?;
        }

        Ok(())
    }

    /// The caller must ensure the following, otherwise data may corrupt:
    ///
    /// - The generator is not in the possesion of any current row.
    /// - The index of the first row is even.
    /// - The rows are the same length.
    /// - each row must have the appropriate size i.e., (page_width * PAGE_STRIDE + 2 * BORDER_SIZE) * bytes_per_texel * PAGE_SIZE
    fn mip_two_rows(
        &mut self,
        rows: (&[u8], &[u8]),
        first_index: usize,
        storage: &mut TextureStorage,
    ) -> Result<(), TextureStorageError> {
        use image::{imageops::resize, ImageBuffer, Rgba};
        debug_assert!(self.stored_row.is_none());
        debug_assert!(first_index % 2 == 0);
        debug_assert!(rows.0.len() == rows.1.len());
        debug_assert!(rows.0.len() % PAGE_SIZE == 0);

        // Current row width
        let row_width = rows.0.len() / PAGE_SIZE;
        let row_texel_width = row_width / self.bytes_per_texel as usize;

        // Border bounds
        let horizontal_border_size = PAGE_BORDER_SIZE * row_width;
        let bottom_border_start = rows.0.len() - horizontal_border_size;
        let top_border_end = horizontal_border_size;

        // New dimensions
        let new_width =
            (row_texel_width as u32 / 2 + PAGE_BORDER_SIZE as u32).max(PAGE_SIZE as u32);
        let new_height = PAGE_SIZE as u32 / 2;

        // Mipping process
        let top_image = ImageBuffer::<Rgba<u8>, &[u8]>::from_raw(
            row_texel_width as u32,
            (PAGE_SIZE - PAGE_BORDER_SIZE) as u32,
            &rows.0[..bottom_border_start],
        )
        .unwrap();
        let bottom_image = ImageBuffer::<Rgba<u8>, _>::from_raw(
            row_texel_width as u32,
            (PAGE_SIZE - 4) as u32,
            &rows.1[top_border_end..],
        )
        .unwrap();
        let mipped_top = resize(&top_image, new_width, new_height, self.filter_mode);
        let mipped_bottom = resize(&bottom_image, new_width, new_height, self.filter_mode);
        let mut mipped_buffer = mipped_top.into_raw();
        mipped_buffer.extend_from_slice(&mipped_bottom.into_raw());
        let mipped_row = mipped_buffer.into_boxed_slice();

        // Write to higher mip level
        if let Some(ref mut next_mip) = self.next_mip {
            next_mip.write_row(mipped_row, first_index / 2, storage)?;
        }

        Ok(())
    }

    /// Writes two rows at once.
    ///
    /// This allows some checks and the allocation on the heap to be skipped for the current mip level.
    pub fn write_two_rows(
        &mut self,
        rows: (&[u8], &[u8]),
        first_index: usize,
        storage: &mut TextureStorage,
    ) -> Result<(), TextureStorageError> {
        storage.write_row(self.mip_level, first_index as u16, rows.0)?;
        storage.write_row(self.mip_level, first_index as u16 + 1, rows.1)?;
        self.mip_two_rows(rows, first_index, storage)?;
        Ok(())
    }
}
