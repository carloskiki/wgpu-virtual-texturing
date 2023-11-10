use crate::storage::mip_generator::MipLevelGen;
use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

mod mip_generator;

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

    /// Creates a new texture storage manager in the directory provided  with '{metadata_file}.json' as the metadata file (Default: "meta").
    /// - `name` (Default: "CARGO_MANIFEST_DIR/texture"): The directory that will contain the texture.
    /// - `metadata_file` (Default: "meta"): The name of the metadata file for the texture.
    ///
    /// ### Errors
    ///
    /// - If the directory that will contain the texture already exists.
    /// - If the directory could not be created
    /// - If the metadata file could not be created/opened.
    pub fn new(
        metadata: TextureMetadata,
        name: Option<&str>,
        metadata_file: Option<&str>,
    ) -> Result<Self, TextureStorageError> {
        let directory = PathBuf::from(name.unwrap_or(Self::DEFAULT_DIRECTORY));

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
        let page_count = (data.len() / PAGE_SIZE / self.metadata.bytes_per_texel as usize
            - 2 * PAGE_BORDER_SIZE)
            / PAGE_STRIDE;
        assert_eq!(page_count, (self.metadata.dimensions.0 >> mip) as usize);
        let texture_texel_width = data.len() / self.metadata.bytes_per_texel as usize / PAGE_SIZE;

        let mut file = self.open_row_file(
            mip,
            row,
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true),
        )?;
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
        log::debug!("wrote row {} of mip level {}", row, mip);

        Ok(())
    }

    /// Import a new texture from a [`Read`] stream of bytes
    ///
    /// - `fit_operation`: The operation to perform if the texture does not fit in the texture storage.
    /// if set to `None`, the texture must have power of two sidelengths (e.g., 4096x1024).
    pub fn import_texture(
        &mut self,
        filter_mode: image::imageops::FilterType,
        mut byte_stream: impl Read,
    ) -> Result<(), TextureStorageError> {
        let texture_dimensions = self.metadata.dimensions;
        let texture_texel_width =
            texture_dimensions.0 as usize * PAGE_STRIDE + 2 * PAGE_BORDER_SIZE;
        let buffer_border_offset =
            texture_texel_width * PAGE_BORDER_SIZE * 2 * self.metadata.bytes_per_texel as usize;

        let mut buffer: Vec<u8> = vec![
            0;
            self.metadata.bytes_per_texel as usize
                * texture_texel_width
                * (PAGE_STRIDE * 2 + PAGE_BORDER_SIZE * 2)
        ];

        let mut mipmap_generator = MipLevelGen::from_mip(
            self.metadata.mip_levels,
            0,
            self.metadata.bytes_per_texel,
            filter_mode,
        );

        // Read top border in
        byte_stream.read_exact(&mut buffer[..buffer_border_offset])?;

        (0..texture_dimensions.1 / 2).try_for_each(|half_texture_row| {
            // Read in the next 2 rows
            byte_stream.read_exact(&mut buffer[buffer_border_offset..])?;

            let page_size_rows =
                PAGE_SIZE * texture_texel_width * self.metadata.bytes_per_texel as usize;
            let first_row = &buffer[0..page_size_rows];
            let second_row_start = buffer.capacity() - page_size_rows;
            let second_row = &buffer[second_row_start..];

            // Write 2 rows
            mipmap_generator.write_two_rows(
                (first_row, second_row),
                half_texture_row as usize * 2,
                self,
            )?;

            // Move bottom border to top border
            let bottom_border = buffer.capacity() - buffer_border_offset;
            buffer.copy_within(bottom_border.., 0);

            Ok::<(), TextureStorageError>(())
        })?;

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
            .map_err(TextureStorageError::from)
    }
}


#[derive(Error, Debug)]
pub enum TextureStorageError {
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error(
        "could not parse metadata file, this should only occur if the file was edited manually"
    )]
    Deserialization(#[from] miniserde::Error),
}

#[derive(MiniSerialize, Deserialize)]
pub struct TextureMetadata {
    dimensions: (u16, u16),
    bytes_per_texel: u8,
    mip_levels: u8,
    // ecoding
}

impl TextureMetadata {
    const MAX_TEXTURE_SIZE: u16 = 1 << 12;

    /// Creates a texture from the provided number of pages per side and bytes per texel.
    ///
    /// If the number of pages is not a power of two, the next power of two will be used.
    ///
    /// ### Panics
    ///
    /// - If any of the sides is bigger than 4096 (2^12).
    /// - If any of the sides is not a power of two.
    pub fn from_dimensions(dimensions: (u16, u16), bytes_per_texel: u8) -> Self {
        assert!(dimensions.0 <= Self::MAX_TEXTURE_SIZE);
        assert!(dimensions.1 <= Self::MAX_TEXTURE_SIZE);
        assert!(dimensions.0.is_power_of_two());
        assert!(dimensions.1.is_power_of_two());
        // We only support RGBA8 textures for now
        assert!(bytes_per_texel == 4);
        let longest_side = dimensions.0.max(dimensions.1);
        let mip_levels = longest_side.ilog2() as u8;

        Self {
            dimensions,
            bytes_per_texel,
            mip_levels,
        }
    }

    /// Creates a square texture from the mip level.
    /// 
    /// ### Panics
    ///
    /// - If the mip level is bigger than lg(MAX_TEXTURE_SIZE).
    pub fn from_mip(mip_levels: u8, bytes_per_texel: u8) -> Self {
        assert!(mip_levels <= Self::MAX_TEXTURE_SIZE.ilog2() as u8);
        // We only support RGBA8 textures for now
        assert!(bytes_per_texel == 4);
        let page_size = 1 << mip_levels;

        Self {
            dimensions: (page_size, page_size),
            bytes_per_texel,
            mip_levels,
        }
    }
}

// fn next_power_of_two(mut n: u16) -> u16 {
//     n -= 1;
//     n |= n >> 1; // Divide by 2^k for consecutive doublings of k up to 32,
//     n |= n >> 2; // and then or the results.
//     n |= n >> 4;
//     n |= n >> 8;
//     n + 1 // The result is a number of 1 bits equal to the number
//           // of bits in the original number, plus 1. That's the
//           // next highest power of 2.
// }

#[cfg(test)]
mod test {
    use std::io::{repeat, Read};

    use assert_fs::{fixture::TempDir, prelude::*};
    use predicates::prelude::*;

    use super::{TextureMetadata, TextureStorage, PAGE_BORDER_SIZE, PAGE_STRIDE};

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
            .write_str(r#"{"dimensions": [16, 16], "bytes_per_texel": 4, "mip_levels": 4}"#)
            .unwrap();

        let _ = TextureStorage::load(Some(path), None).unwrap();
    }

    #[test]
    fn store_256_texture() -> Result<(), Box<dyn std::error::Error>> {
        env_logger::init();
        let (mut texture_storage, _temp_dir) = texture_storage_from_mip(256_usize.ilog2() as u8);
        let bytes =
            repeat(0xFF).take(((256 * PAGE_STRIDE + 2 * PAGE_BORDER_SIZE).pow(2) * 4) as u64);
        texture_storage.import_texture(image::imageops::FilterType::Nearest, bytes)?;

        Ok(())
    }

    fn texture_storage_from_mip(mip_levels: u8) -> (TextureStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().as_os_str().to_str().unwrap();
        let storage =
            TextureStorage::new(TextureMetadata::from_mip(mip_levels, 4), Some(path), None)
                .unwrap();
        (storage, temp_dir)
    }
}
