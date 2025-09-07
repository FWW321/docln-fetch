use anyhow::Result;
use std::path::Path;
use serde::{Serialize, Deserialize};
use super::Epub;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub title: String,
    pub url: String,
    pub has_illustrations: bool, // 是否包含插图
    pub xhtml_path: Option<String>, // XHTML文件路径（用于EPUB）
}

impl Chapter {
    pub fn builder() -> ChapterBuilder {
        ChapterBuilder::new()
    }
}

pub struct ChapterBuilder {
    title: String,
    url: String,
    has_illustrations: bool,
    xhtml_path: Option<String>,
}

impl ChapterBuilder {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            url: String::new(),
            has_illustrations: false,
            xhtml_path: None,
        }
    }

    pub fn title(mut self, title: String) -> Self {
        self.title = title;
        self
    }

    pub fn url(mut self, url: String) -> Self {
        self.url = url;
        self
    }

    pub fn has_illustrations(mut self, has_illustrations: bool) -> Self {
        self.has_illustrations = has_illustrations;
        self
    }

    pub fn xhtml_path(mut self, path: Option<String>) -> Self {
        self.xhtml_path = path;
        self
    }

    pub fn build(self) -> Chapter {
        Chapter {
            title: self.title,
            url: self.url,
            has_illustrations: self.has_illustrations,
            xhtml_path: self.xhtml_path,
        }
    }
}

pub fn generate_all_volume_cover_chapters(epub: &Epub, oebps_dir: &Path) -> Result<()> {
    for (i, volume) in epub.volumes.iter().enumerate() {
        if volume.cover_image_path.is_some() {
            volume.generate_volume_cover_chapter(i, oebps_dir)?;
        }
    }
    Ok(())
}