pub mod parser;
pub mod downloader;
pub mod processor;

pub use parser::NovelParser;
pub use downloader::ImageDownloader;
pub use processor::ChapterProcessor;

use anyhow::Result;
use reqwest;
use scraper::Html;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub title: String,
    pub url: String,
    pub has_illustrations: bool, // 是否包含插图
    pub xhtml_path: Option<String>, // XHTML文件路径（用于EPUB）
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub title: String,
    pub volume_id: String,
    pub cover_image_path: Option<String>,
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovelInfo {
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

pub struct DoclnCrawler {
    client: reqwest::Client,
    base_url: String,
    parser: NovelParser,
    image_downloader: ImageDownloader,
}

impl DoclnCrawler {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .unwrap();

        Self {
            client: client.clone(),
            base_url: "https://docln.net".to_string(),
            parser: NovelParser,
            image_downloader: ImageDownloader::new(client),
        }
    }

    pub async fn fetch_novel_info(&self, novel_id: u32) -> Result<NovelInfo> {
        let url = format!("{}/sang-tac/{}", self.base_url, novel_id);
        
        println!("正在获取: {}", url);
        
        let response = self.client.get(&url).send().await?;
        let html_content = response.text().await?;
        
        self.parse_novel_info(&html_content, &url, novel_id).await
    }

    pub async fn parse_novel_info(&self, html_content: &str, url: &str, novel_id: u32) -> Result<NovelInfo> {
        let document = Html::parse_document(html_content);
        
        // 解析基本信息
        let mut novel_info = self.parser.parse_novel_info(html_content, url, novel_id)?;
        
        // 创建EPUB标准目录结构
        let epub_dir_name = format!("epub_{}", novel_id);
        let epub_dir = std::path::Path::new(&epub_dir_name);
        
        // 解析并下载封面图片
        if let Some(cover_url) = self.parser.extract_cover_url(&document) {
            match self.image_downloader.download_novel_cover(&cover_url, novel_id, &novel_info.title, epub_dir).await {
                Ok(Some(path)) => novel_info.cover_image_path = Some(path),
                Ok(None) => println!("使用默认封面图片，跳过下载"),
                Err(e) => println!("下载封面图片失败: {}", e),
            }
        }
        
        // 解析卷信息
        let volume_infos = self.parser.parse_volume_info(&document);
        let mut volumes = Vec::new();
        
        for (volume_index, (volume_title, volume_id)) in volume_infos.iter().enumerate() {
            // 解析该卷的章节信息
            let mut chapters = self.parser.parse_volume_chapters(&document, volume_id);
            
            // 查找卷封面图片
            let mut volume_cover_path = None;
            if let Some(cover_url) = self.parser.extract_volume_cover_url(&document, volume_id) {
                match self.image_downloader.download_volume_cover_image(&cover_url, volume_index, volume_title, epub_dir).await {
                    Ok(path) => volume_cover_path = path,
                    Err(e) => println!("下载卷 '{}' 封面图片失败: {}", volume_title, e),
                }
            }
            
            // 处理该卷的章节内容
            if !chapters.is_empty() {
                println!("\n正在处理卷 '{}' 的 {} 个章节...", volume_title, chapters.len());
                
                // 创建EPUB标准的images目录
                let images_dir = epub_dir.join("OEBPS").join("images");
                std::fs::create_dir_all(&images_dir)?;
                
                let chapter_processor = ChapterProcessor::new(self.client.clone(), self.base_url.clone());
                match chapter_processor.fetch_and_process_chapters(
                    &mut chapters,
                    volume_index,
                    volume_title,
                    &novel_info.title,
                    &images_dir,
                ).await {
                    Ok(()) => println!("卷 '{}' 章节处理完成", volume_title),
                    Err(e) => println!("处理卷 '{}' 章节时出错: {}", volume_title, e),
                }
            }
            
            volumes.push(Volume {
                title: volume_title.to_string(),
                volume_id: volume_id.clone(),
                cover_image_path: volume_cover_path,
                chapters,
            });
        }
        
        novel_info.volumes = volumes;
        
        // 生成EPUB文件
        match crate::epub::EpubBuilder::new()
            .novel_info(novel_info.clone())
            .epub_dir(epub_dir_name)
            .build_async().await {
            Ok(epub_filename) => {
                println!("EPUB文件生成成功: {}", epub_filename);
            }
            Err(e) => {
                println!("压缩EPUB文件失败: {}", e);
            }
        }

        Ok(novel_info)
    }

    pub async fn crawl_novel(&self, novel_id: u32) {
        match self.fetch_novel_info(novel_id).await {
            Ok(novel_info) => {
                println!("\n=== 小说信息 ===");
                println!("标题: {}", novel_info.title);
                println!("作者: {}", novel_info.author);
                if let Some(illustrator) = &novel_info.illustrator {
                    println!("插画师: {}", illustrator);
                }
                if !novel_info.summary.is_empty() {
                    println!("简介: {}", novel_info.summary);
                }
                if let Some(cover_path) = &novel_info.cover_image_path {
                    println!("封面: {}", cover_path);
                } else {
                    println!("封面: 使用默认封面");
                }
                println!("标签: {}", novel_info.tags.join(", "));
                
                // 显示卷信息
                if !novel_info.volumes.is_empty() {
                    println!("\n目录结构:");
                    for (i, volume) in novel_info.volumes.iter().enumerate() {
                        println!("  ├── {} (卷 {})", volume.title, i + 1);
                        if !volume.chapters.is_empty() {
                            let processed_count = volume.chapters.iter().filter(|c| c.xhtml_path.is_some()).count();
                            if processed_count > 0 {
                                let display_count = std::cmp::min(3, processed_count);
                                let mut displayed = 0;
                                for chapter in &volume.chapters {
                                    if let Some(_) = &chapter.xhtml_path {
                                        if displayed < display_count {
                                            let chapter_prefix = if chapter.has_illustrations { "📄" } else { "📖" };
                                            println!("  │   ├── {} {}", chapter_prefix, chapter.title);
                                            displayed += 1;
                                        }
                                    }
                                }
                                if processed_count > display_count {
                                    println!("  │   └── ... (还有 {} 个章节)", processed_count - display_count);
                                }
                            }
                        }
                        if i < novel_info.volumes.len() - 1 {
                            println!("  │");
                        }
                    }
                }
                
                println!("URL: {}", novel_info.url);
                println!("==============\n");
            }
            Err(e) => {
                println!("爬取小说失败 (ID: {}): {}", novel_id, e);
            }
        }
    }
}