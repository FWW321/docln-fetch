pub mod compression;
pub mod metadata;
pub mod chapter;

pub use compression::EpubCompressor;
pub use metadata::MetadataGenerator;
pub use chapter::ChapterGenerator;

use anyhow::Result;
use std::path::Path;
use crate::crawler::NovelInfo;

pub struct EpubBuilder {
    novel_info: Option<NovelInfo>,
    epub_dir: Option<String>,
}

impl EpubBuilder {
    pub fn new() -> Self {
        Self {
            novel_info: None,
            epub_dir: None,
        }
    }

    pub fn novel_info(mut self, novel_info: NovelInfo) -> Self {
        self.novel_info = Some(novel_info);
        self
    }

    pub fn epub_dir<S: Into<String>>(mut self, epub_dir: S) -> Self {
        self.epub_dir = Some(epub_dir.into());
        self
    }

    pub fn build(self) -> Result<()> {
        let novel_info = self.novel_info.ok_or_else(|| anyhow::anyhow!("Novel info is required"))?;
        let epub_dir = self.epub_dir.ok_or_else(|| anyhow::anyhow!("EPUB directory is required"))?;

        let metadata_generator = MetadataGenerator::new();
        let chapter_generator = ChapterGenerator::new();
        
        let epub_path = Path::new(&epub_dir);
        
        // 生成所有元数据文件
        metadata_generator.generate_all_metadata(&novel_info, epub_path, novel_info.id)?;
        
        // 生成卷封面章节
        let oebps_dir = epub_path.join("OEBPS");
        chapter_generator.generate_all_volume_cover_chapters(&novel_info, &oebps_dir)?;
        
        println!("EPUB元数据文件已生成");
        Ok(())
    }

    pub async fn build_async(self) -> Result<String> {
        let novel_info = self.novel_info.ok_or_else(|| anyhow::anyhow!("Novel info is required"))?;
        let epub_dir = self.epub_dir.ok_or_else(|| anyhow::anyhow!("EPUB directory is required"))?;

        let metadata_generator = MetadataGenerator::new();
        let chapter_generator = ChapterGenerator::new();
        
        let epub_path = Path::new(&epub_dir);
        
        // 生成所有元数据文件
        metadata_generator.generate_all_metadata(&novel_info, epub_path, novel_info.id)?;
        
        // 生成卷封面章节
        let oebps_dir = epub_path.join("OEBPS");
        chapter_generator.generate_all_volume_cover_chapters(&novel_info, &oebps_dir)?;
        
        // 压缩成EPUB文件
        let compressor = EpubCompressor::new();
        let epub_filename = compressor.compress_epub(epub_path)?;
        
        println!("EPUB文件生成成功: {}", epub_filename);
        Ok(epub_filename)
    }
}