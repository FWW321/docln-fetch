pub mod compression;
pub mod metadata;
pub mod chapter;
pub mod volume;

pub use compression::EpubCompressor;
pub use metadata::MetadataGenerator;
pub use volume::{Volume, VolumeBuilder};
pub use chapter::{Chapter, ChapterBuilder};

use anyhow::Result;
use std::path::Path;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Epub {
    pub id: u32,
    pub title: String,
    pub author: String,
    pub illustrator: Option<String>, // 插画师
    pub summary: String, // 简介内容
    pub cover_image_path: Option<String>, // 封面图片本地路径
    pub volumes: Vec<Volume>, // 卷信息
    pub tags: Vec<String>,
    pub url: String,
}

pub struct EpubGenerator {
    epub: Epub,
    epub_dir: Option<String>,
}

impl EpubGenerator {
    pub fn new(epub: Epub) -> Self {
        Self {
            epub,
            epub_dir: None,
        }
    }

    pub fn epub_dir<S: Into<String>>(mut self, epub_dir: S) -> Self {
        self.epub_dir = Some(epub_dir.into());
        self
    }

    pub fn generate(self) -> Result<String> {
        let epub_dir = self.epub_dir.ok_or_else(|| anyhow::anyhow!("EPUB directory is required"))?;

        // 创建 EPUB 结构体

        let metadata_generator = MetadataGenerator::new();
        
        let epub_path = Path::new(&epub_dir);
        
        // 生成所有元数据文件
        metadata_generator.generate_all_metadata(&self.epub, epub_path, self.epub.id)?;
        
        // 生成卷封面章节
        let oebps_dir = epub_path.join("OEBPS");
        crate::epub::chapter::generate_all_volume_cover_chapters(&self.epub, &oebps_dir)?;
        
        // 压缩成EPUB文件
        let compressor = EpubCompressor::new();
        let epub_filename = compressor.compress_epub(epub_path)?;
        
        println!("EPUB文件生成成功: {}", epub_filename);
        Ok(epub_filename)
    }
}