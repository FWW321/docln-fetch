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

impl Epub {
    pub fn builder() -> EpubBuilder {
        EpubBuilder::new()
    }
}

pub struct EpubBuilder {
    id: u32,
    title: String,
    author: String,
    illustrator: Option<String>,
    summary: String,
    cover_image_path: Option<String>,
    volumes: Vec<Volume>,
    tags: Vec<String>,
    url: String,
    epub_dir: Option<String>,
}

impl EpubBuilder {
    pub fn new() -> Self {
        Self {
            id: 0,
            title: String::new(),
            author: String::new(),
            illustrator: None,
            summary: String::new(),
            cover_image_path: None,
            volumes: Vec::new(),
            tags: Vec::new(),
            url: String::new(),
            epub_dir: None,
        }
    }

    pub fn id(mut self, id: u32) -> Self {
        self.id = id;
        self
    }

    pub fn title(mut self, title: String) -> Self {
        self.title = title;
        self
    }

    pub fn author(mut self, author: String) -> Self {
        self.author = author;
        self
    }

    pub fn url(mut self, url: String) -> Self {
        self.url = url;
        self
    }

    pub fn illustrator(mut self, illustrator: Option<String>) -> Self {
        self.illustrator = illustrator;
        self
    }

    pub fn summary(mut self, summary: String) -> Self {
        self.summary = summary;
        self
    }

    pub fn cover_image_path(mut self, path: Option<String>) -> Self {
        self.cover_image_path = path;
        self
    }

    pub fn volumes(mut self, volumes: Vec<Volume>) -> Self {
        self.volumes = volumes;
        self
    }

    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn epub_dir<S: Into<String>>(mut self, epub_dir: S) -> Self {
        self.epub_dir = Some(epub_dir.into());
        self
    }

    pub fn build(self) -> Result<String> {
        let epub_dir = self.epub_dir.ok_or_else(|| anyhow::anyhow!("EPUB directory is required"))?;

        // 创建 EPUB 结构体
        let epub = Epub {
            id: self.id,
            title: self.title,
            author: self.author,
            illustrator: self.illustrator,
            summary: self.summary,
            cover_image_path: self.cover_image_path,
            volumes: self.volumes,
            tags: self.tags,
            url: self.url,
        };

        let metadata_generator = MetadataGenerator::new();
        
        let epub_path = Path::new(&epub_dir);
        
        // 生成所有元数据文件
        metadata_generator.generate_all_metadata(&epub, epub_path, epub.id)?;
        
        // 生成卷封面章节
        let oebps_dir = epub_path.join("OEBPS");
        crate::epub::chapter::generate_all_volume_cover_chapters(&epub, &oebps_dir)?;
        
        // 压缩成EPUB文件
        let compressor = EpubCompressor::new();
        let epub_filename = compressor.compress_epub(epub_path)?;
        
        println!("EPUB文件生成成功: {}", epub_filename);
        Ok(epub_filename)
    }
}