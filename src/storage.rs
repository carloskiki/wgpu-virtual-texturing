use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

use miniserde::{Deserialize, MiniSerialize};
use thiserror::Error;

const PAGE_SIZE: usize = 128;
const PAGE_STRIDE: usize = PAGE_SIZE - 2 * PAGE_BORDER_SIZE;
const PAGE_BORDER_SIZE: usize = 4;

pub struct TextureStorage {
    directory: std::path::PathBuf,
    metadata: TextureMetadata,
}

impl TextureStorage {
    const DEFAULT_DIRECTORY: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/texture");
    const DEFAULT_METADATA_FILE: &'static str = "meta";

    /// Creates a new texture storage manager in the directory provided (Default:
    /// "CARGO_MANIFEST_DIR/texture") with '{metadata_file}.json' as the metadata file (Default: "meta").
    ///
    /// ### Errors
    ///
    /// - If the directory that will contain the texture already exists.
    /// - If the directory could not be created or if the metadata file
    /// could not be created or opened.
    pub fn new(
        metadata: TextureMetadata,
        directory: Option<&str>,
        metadata_file: Option<&str>,
    ) -> Result<Self, TextureStorageError> {
        let directory = PathBuf::from(directory.unwrap_or(Self::DEFAULT_DIRECTORY));

        std::fs::create_dir_all(&directory)?;

        let mut meta_file = File::create(directory.join(format!(
            "{}.json",
            metadata_file.unwrap_or(Self::DEFAULT_METADATA_FILE)
        )))?;
        meta_file.write_all(miniserde::json::to_string(&metadata).as_bytes())?;

        Ok(Self {
            directory,
            metadata,
        })
    }

    /// Load an existing texture from the directory provided (Default: "CARGO_MANIFEST_DIR/texture") with
    /// '{metadata_file}.json' as the metadata file (Default: "meta").
    pub fn load(
        directory: Option<&str>,
        metadata_file: Option<&str>,
    ) -> Result<Self, TextureStorageError> {
        let directory = PathBuf::from(directory.unwrap_or(Self::DEFAULT_DIRECTORY));

        let mut meta_file = File::open(directory.join(format!(
            "{}.json",
            metadata_file.unwrap_or(Self::DEFAULT_METADATA_FILE)
        )))?;

        let mut metadata_string = String::new();
        meta_file.read_to_string(&mut metadata_string)?;

        let metadata: TextureMetadata = miniserde::json::from_str(&metadata_string)?;

        Ok(Self {
            directory,
            metadata,
        })
    }

    fn write_row(&mut self, mip: u8, row: u16, data: &[u8]) -> Result<(), TextureStorageError> {
        let page_count = (data.len() / self.metadata.bytes_per_texel as usize / PAGE_SIZE
            - 2 * PAGE_BORDER_SIZE)
            / PAGE_STRIDE;
        assert_eq!(page_count, (self.metadata.side_len >> mip) as usize);
        let texture_texel_width = data.len() / self.metadata.bytes_per_texel as usize / PAGE_SIZE;

        let mut file = self.open_row_file(mip, row, std::fs::OpenOptions::new().truncate(true))?;
        (0..page_count).try_for_each(|page| {
            let column_offset = page * PAGE_STRIDE;
            (0..PAGE_SIZE).try_for_each(|page_row| -> Result<(), TextureStorageError> {
                let start = column_offset + page_row * texture_texel_width;
                let end = start + PAGE_SIZE;
                file.write_all(&data[start..end])?;
                Ok(())
            })?;
            Ok::<(), TextureStorageError>(())
        })?;

        Ok(())
    }

    pub fn import_texture(
        &mut self,
        mut byte_stream: impl Read,
    ) -> Result<(), TextureStorageError> {
        let texture_side_len = self.metadata.side_len;
        let texture_texel_width = texture_side_len as usize * PAGE_STRIDE + 2 * PAGE_BORDER_SIZE;
        let buffer_border_offset = texture_texel_width * PAGE_BORDER_SIZE;

        let mut buffer: Vec<u8> = Vec::with_capacity(
            self.metadata.bytes_per_texel as usize
                * texture_texel_width
                * (PAGE_STRIDE * 2 + PAGE_BORDER_SIZE * 2),
        );

        let mut mipmap_generator = MipLevelGen::from_mip(self.metadata.mip_levels, 0);

        // Read top border in
        byte_stream.read_exact(&mut buffer[..buffer_border_offset])?;

        (0..texture_side_len / 2).try_for_each(|half_texture_row| {
            // Read in the next 2 rows
            byte_stream.read_exact(&mut buffer[buffer_border_offset..])?;

            let page_size_rows = PAGE_SIZE * texture_texel_width;
            let first_row = &buffer[0..page_size_rows];
            let second_row_start = buffer.capacity() - page_size_rows;
            let second_row = &buffer[second_row_start..];

            // Write 2 rows
            mipmap_generator.write_two_rows(
                (first_row, second_row),
                half_texture_row as usize * 2,
                self,
            )?;

            Ok::<(), TextureStorageError>(())
        })?;

        // Move bottom border to top border
        let bottom_border = buffer.capacity() - buffer_border_offset;
        buffer.copy_within(bottom_border.., 0);

        Ok(())
    }

    fn open_row_file(
        &mut self,
        mip: u8,
        row: u16,
        opts: &std::fs::OpenOptions,
    ) -> Result<std::fs::File, TextureStorageError> {
        let file_name = format!("{}-{}", mip, row);
        opts.open(self.directory.join(file_name))
            .map_err(TextureStorageError::IoError)
    }
}

struct MipLevelGen {
    next_mip: Option<Box<MipLevelGen>>,
    stored_row: Option<(Box<[u8]>, usize)>,
    mip_level: u8,
}

impl MipLevelGen {
    /// Creates a new generator
    fn from_mip(mip: u8, base_mip: u8) -> Self {
        let next_mip = (mip < base_mip).then(|| Box::new(Self::from_mip(mip, base_mip + 1)));
        Self {
            stored_row: None,
            mip_level: base_mip,
            next_mip,
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

    fn mip_two_rows(
        &mut self,
        rows: (&[u8], &[u8]),
        first_index: usize,
        storage: &mut TextureStorage,
    ) -> Result<(), TextureStorageError> {
        assert!(self.stored_row.is_none());
        assert!(first_index % 2 == 0);
        let mipped_row = mip_two_rows(rows);
        if let Some(ref mut next_mip) = self.next_mip {
            next_mip.write_row(mipped_row, first_index / 2, storage)?;
        }

        Ok(())
    }

    /// Writes two rows at once.
    ///
    /// This allows some checks to be skipped but most importantly allows the allocation on the
    /// heap to be skipped for the current mip level.
    fn write_two_rows(
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

fn mip_two_rows(rows: (&[u8], &[u8])) -> Box<[u8]> {
    use image::{imageops::resize, ImageBuffer, Rgba};
    assert!(rows.0.len() == rows.1.len());
    // check that there is 128px of height, and that there is a multiple of 2 pages.
    assert!(rows.0.len() % (PAGE_SIZE * 2) == 0);

    let row_width = rows.0.len() / PAGE_SIZE;
    let horizontal_border_size = PAGE_BORDER_SIZE * row_width;
    let bottom_border_start = rows.0.len() - horizontal_border_size;
    let top_border_end = horizontal_border_size;

    let from_image_buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(
        row_width as u32,
        (PAGE_SIZE - 4) as u32,
        &rows.0[0..bottom_border_start],
    )
    .unwrap();

    let mipped_top = resize(
        &from_image_buffer,
        row_width as u32 / 2,
        PAGE_SIZE as u32 / 2,
        image::imageops::FilterType::Nearest,
    );
    let mut mipped_buffer = mipped_top.into_raw();

    let from_image_buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(
        row_width as u32,
        (PAGE_SIZE - 4) as u32,
        &rows.1[top_border_end..],
    )
    .unwrap();
    let mipped_bottom = resize(
        &from_image_buffer,
        row_width as u32 / 2,
        PAGE_SIZE as u32 / 2,
        image::imageops::FilterType::Nearest,
    );
    mipped_buffer.extend_from_slice(&mipped_bottom.into_raw());
    mipped_buffer.into_boxed_slice()
}

#[derive(Error, Debug)]
pub enum TextureStorageError {
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("could not parse metadata file, this can only occur if the file was edited manually")]
    Deserialization(#[from] miniserde::Error),
}

#[derive(MiniSerialize, Deserialize)]
pub struct TextureMetadata {
    side_len: u16,
    bytes_per_texel: u8,
    mip_levels: u8,
    // ecoding
}

impl TextureMetadata {
    const MAX_TEXTURE_SIZE: u16 = 1 << 11;

    /// Creates a texture from the provided number of pages per side and bytes per texel.
    ///
    /// If the number of pages is not a power of two, the next power of two will be used.
    ///
    /// ### Errors
    ///
    /// - Errors if the number of pages is greater than 8192 (2^13).
    pub fn from_dimensions(page_size: u16, bytes_per_texel: u8) -> Self {
        assert!(page_size <= Self::MAX_TEXTURE_SIZE);
        // We only support RGBA8 textures for now
        assert!(bytes_per_texel == 4);
        let page_size = next_power_of_two(page_size);
        let mip_levels = page_size.ilog2() as u8;

        Self {
            side_len: page_size,
            bytes_per_texel,
            mip_levels,
        }
    }

    pub fn from_mip(mip_levels: u8, bytes_per_texel: u8) -> Self {
        assert!(mip_levels <= Self::MAX_TEXTURE_SIZE.ilog2() as u8);
        // We only support RGBA8 textures for now
        assert!(bytes_per_texel == 4);
        let page_size = 1 << mip_levels;

        Self {
            side_len: page_size,
            bytes_per_texel,
            mip_levels,
        }
    }
}

fn next_power_of_two(mut n: u16) -> u16 {
    n -= 1;
    n |= n >> 1; // Divide by 2^k for consecutive doublings of k up to 32,
    n |= n >> 2; // and then or the results.
    n |= n >> 4;
    n |= n >> 8;
    n + 1 // The result is a number of 1 bits equal to the number
          // of bits in the original number, plus 1. That's the
          // next highest power of 2.
}

#[cfg(test)]
mod test {
    use assert_fs::{fixture::TempDir, prelude::*};
    use predicates::prelude::*;

    use super::{TextureMetadata, TextureStorage};

    #[test]
    fn create_texture_storage() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().as_os_str().to_str().unwrap();
        let _ = TextureStorage::new(TextureMetadata::from_mip(4, 4), Some(path), None).unwrap();
        temp_dir
            .child("meta.json")
            .assert(predicate::path::exists());
    }

    #[test]
    fn load_texture_storage() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().as_os_str().to_str().unwrap();
        let metadata = temp_dir.child("meta.json");

        metadata.touch().unwrap();
        metadata
            .write_str(r#"{"side_len": 16, "bytes_per_texel": 4, "mip_levels": 4}"#)
            .unwrap();

        let _ = TextureStorage::load(Some(path), None).unwrap();
    }
}
