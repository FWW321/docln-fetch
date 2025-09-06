use reqwest;
use scraper::{Html, Selector, Element};
use anyhow::Result;
use std::fs;
use std::path::Path;
use crate::models::{NovelInfo, Volume, Chapter};

pub struct DoclnCrawler {
    client: reqwest::Client,
    base_url: String,
}

impl DoclnCrawler {
    /// 通用的图片下载函数
    async fn download_image(
        &self,
        image_url: &str,
        filepath: &Path,
        log_prefix: &str,
    ) -> Result<()> {
        println!("正在下载{}图片: {}", log_prefix, image_url);
        
        // 下载图片
        let response = self.client.get(image_url).send().await?;
        let image_bytes = response.bytes().await?;
        
        // 保存到本地
        fs::write(filepath, &image_bytes)?;
        
        println!("{}图片已保存到: {}", log_prefix, filepath.display());
        Ok(())
    }
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();

        Self {
            client,
            base_url: "https://docln.net".to_string(),
        }
    }

    pub async fn fetch_novel_info(&self, novel_id: u32) -> Result<NovelInfo> {
        let url = format!("{}/sang-tac/{}", self.base_url, novel_id);
        
        println!("正在获取: {}", url);
        
        let response = self.client.get(&url).send().await?;
        let html_content = response.text().await?;
        
        self.parse_novel_info(&html_content, &url, novel_id).await
    }

    async fn download_volume_cover_image(&self, image_url: &str, _volume_index: usize, volume_title: &str, epub_dir: &Path) -> Result<Option<String>> {
        // 从URL中提取文件扩展名
        let extension = Path::new(image_url)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("jpg");
        
        // 清理卷标题中的特殊字符，用于文件名，并添加编号
        let safe_volume_title = volume_title
            .chars()
            .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { '_' })
            .collect::<String>()
            .replace(' ', "_");
        
        // EPUB标准目录结构: OEBPS/images/
        let images_dir = epub_dir.join("OEBPS").join("images");
        fs::create_dir_all(&images_dir)?;
        
        // 卷封面命名为卷名
        let filename = format!("{}.{}", safe_volume_title, extension);
        
        // 使用通用函数下载卷封面图片
        self.download_cover_image_common(image_url, &images_dir, &filename, &format!("卷 '{}' ", volume_title), true).await
    }

    async fn download_chapter_illustrations(
        &self,
        chapter_paragraphs: &[String],
        images_dir: &Path,
        chapter_index: usize,
        volume_index: usize,
        _volume_title: &str,
        _chapter_title: &str,
    ) -> Result<String> {
        let mut modified_paragraphs = Vec::new();
        let mut illustration_counter = 1;
        
        // 直接创建插图目录 - 按卷文件夹组织（因为进入这个函数的章节一定有插图）
        let volume_img_dir = images_dir.join(format!("volume_{:03}", volume_index + 1));
        let chapter_img_dir = volume_img_dir.join(format!("chapter_{:03}", chapter_index + 1));
        fs::create_dir_all(&chapter_img_dir)?;
        let illustrations_dir = Some(chapter_img_dir);
        
        // 处理每个段落
        for p_html in chapter_paragraphs {
            let mut modified_p_html = p_html.clone();
            
            // 解析段落HTML来查找图片
            let p_document = Html::parse_fragment(&p_html);
            let img_selector = Selector::parse("img").unwrap();
            
            // 处理段落中的图片（如果有）
            for img_element in p_document.select(&img_selector) {
                if let Some(img_src) = img_element.value().attr("src") {
                    if !img_src.is_empty() {
                        // 下载图片
                        match self.download_illustration(img_src, illustrations_dir.as_ref().unwrap(), illustration_counter, volume_index, chapter_index).await {
                            Ok(local_path) => {
                                // 替换原始src为本地路径（相对于images目录）
                                let original_img_html = img_element.html();
                                // 确保img标签正确闭合
                                let modified_img_html = if original_img_html.ends_with("/>") {
                                    original_img_html.replace(img_src, &local_path)
                                } else {
                                    original_img_html.replace(img_src, &local_path).replace(">", "/>")
                                };
                                modified_p_html = modified_p_html.replace(&original_img_html, &modified_img_html);
                                
                                illustration_counter += 1;
                            },
                            Err(e) => {
                                println!("下载插图失败: {}", e);
                            }
                        }
                    }
                }
            }
            
            modified_paragraphs.push(modified_p_html);
        }
        
        Ok(modified_paragraphs.join("\n"))
    }

    async fn download_illustration(
        &self,
        image_url: &str,
        illustrations_dir: &Path,
        illustration_number: usize,
        volume_index: usize,
        chapter_index: usize,
    ) -> Result<String> {
        // 从URL中提取文件扩展名
        let extension = Path::new(image_url)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("jpg");
        
        // 插图命名为顺序编号
        let filename = format!("{:03}.{}", illustration_number, extension);
        let filepath = illustrations_dir.join(&filename);
        
        // 使用通用函数下载图片
        self.download_image(image_url, &filepath, &format!("插图 {}", illustration_number)).await?;
        
        // 返回正确的相对路径（从text/volume_XXX/chapter_XXX.xhtml到images/volume_XXX/chapter_XXX/）
        Ok(format!("../../images/volume_{:03}/chapter_{:03}/{}", volume_index + 1, chapter_index + 1, filename))
    }

    /// 通用的封面图片下载函数
    async fn download_cover_image_common(
        &self,
        image_url: &str,
        images_dir: &Path,
        filename: &str,
        log_prefix: &str,
        skip_default: bool,
    ) -> Result<Option<String>> {
        // 检查是否为默认的nocover图片
        if skip_default && image_url.contains("nocover") {
            println!("{}使用默认封面图片，跳过下载", log_prefix);
            return Ok(None);
        }
        
        let filepath = images_dir.join(filename);
        
        // 使用通用函数下载图片
        self.download_image(image_url, &filepath, log_prefix).await?;
        
        println!("{}封面图片已保存到: {} (文件名: {})", log_prefix, filepath.display(), filename);
        
        // 返回相对路径（相对于OEBPS目录）
        Ok(Some(format!("images/{}", filename)))
    }

    pub async fn fetch_chapter_content(
        &self,
        chapter_url: &str,
        volume_index: usize,
        chapter_index: usize,
        volume_title: &str,
        chapter_title: &str,
        images_dir: &Path,
        has_illustrations: bool,
    ) -> Result<String> {
        println!("正在获取章节内容: {}", chapter_url);
        
        let response = self.client.get(chapter_url).send().await?;
        let html_content = response.text().await?;
        
        let document = Html::parse_document(&html_content);
        
        // 提取章节内容
        let chapter_content_selector = Selector::parse("div#chapter-content").unwrap();
        let mut chapter_paragraphs = Vec::new();
        
        if let Some(content_div) = document.select(&chapter_content_selector).next() {
            // 获取所有段落
            let p_selector = Selector::parse("p").unwrap();
            for p_element in content_div.select(&p_selector) {
                chapter_paragraphs.push(p_element.html());
            }
        }
        
        // 根据章节是否有插图决定是否处理图片
        let modified_content = if has_illustrations {
            self.download_chapter_illustrations(
                &chapter_paragraphs,
                images_dir,
                chapter_index,
                volume_index,
                volume_title,
                chapter_title,
            ).await?
        } else {
            // 没有插图，直接使用原始段落内容
            chapter_paragraphs.join("\n")
        };
        
        // 创建XHTML内容 - 在body下创建div容器
        let mut xhtml_content = String::new();
        
        // XHTML头部
        xhtml_content.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
    <title>"#);
        xhtml_content.push_str(chapter_title);
        xhtml_content.push_str(r#"</title>
    <meta http-equiv="Content-Type" content="text/html; charset=UTF-8"/>
</head>
<body>
    <h1>"#);
        xhtml_content.push_str(chapter_title);
        xhtml_content.push_str(r#"</h1>
    <div class="chapter-content">
"#);
        
        // 添加章节内容
        xhtml_content.push_str(&modified_content);
        
        // XHTML尾部
        xhtml_content.push_str(r#"    </div>
</body>
</html>"#);
        
        // 保存XHTML文件 - 按卷文件夹组织
        let volume_dir = images_dir.parent().unwrap().join("text").join(format!("volume_{:03}", volume_index + 1));
        fs::create_dir_all(&volume_dir)?;
        
        let xhtml_filename = format!("chapter_{:03}.xhtml", chapter_index + 1);
        let xhtml_path = volume_dir.join(&xhtml_filename);
        fs::write(&xhtml_path, xhtml_content)?;
        
        println!("章节 XHTML 已保存到: {}", xhtml_path.display());
        
        // 返回相对路径（相对于OEBPS目录）
        Ok(format!("text/volume_{:03}/{}", volume_index + 1, xhtml_filename))
    }

    fn parse_volume_chapters(
        &self,
        document: &Html,
        volume_id: &str,
        _novel_id: u32,
        _volume_title: &str,
    ) -> Vec<Chapter> {
        let mut chapters = Vec::new();
        
        // 根据volume_id找到对应的卷元素
        let volume_element_id = volume_id.trim_start_matches('#');
        let volume_header_selector = Selector::parse(&format!("header#{}", volume_element_id)).unwrap();
        let list_chapters_selector = Selector::parse("ul.list-chapters").unwrap();
        let chapter_item_selector = Selector::parse("li").unwrap();
        let chapter_name_selector = Selector::parse("div.chapter-name").unwrap();
        let chapter_link_selector = Selector::parse("a").unwrap();
        let illustration_icon_selector = Selector::parse("i").unwrap();
        
        if let Some(volume_header) = document.select(&volume_header_selector).next() {
            if let Some(parent_element) = volume_header.parent_element() {
                // 在该卷元素中查找章节列表
                if let Some(chapters_list) = parent_element.select(&list_chapters_selector).next() {
                    for chapter_item in chapters_list.select(&chapter_item_selector) {
                        // 查找章节名称和链接
                        if let Some(chapter_name_div) = chapter_item.select(&chapter_name_selector).next() {
                            if let Some(chapter_link) = chapter_name_div.select(&chapter_link_selector).next() {
                                let chapter_title = chapter_link
                                    .text()
                                    .collect::<String>()
                                    .trim()
                                    .to_string();
                                
                                let chapter_url = chapter_link
                                    .value()
                                    .attr("href")
                                    .unwrap_or("")
                                    .to_string();
                                
                                // 检查是否包含插图图标
                                let has_illustrations = chapter_name_div.select(&illustration_icon_selector).next().is_some();
                                
                                if !chapter_title.is_empty() && !chapter_url.is_empty() {
                                    chapters.push(Chapter {
                                        title: chapter_title,
                                        url: chapter_url,
                                        has_illustrations,
                                        xhtml_path: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        
        chapters
    }

    pub async fn fetch_and_process_chapters(
        &self,
        chapters: &mut Vec<Chapter>,
        volume_index: usize,
        volume_title: &str,
        _novel_title: &str,
        images_dir: &Path,
    ) -> Result<()> {
        println!("\n正在处理卷 '{}' 的章节内容...", volume_title);
        
        for (chapter_index, chapter) in chapters.iter_mut().enumerate() {
            let full_chapter_url = if chapter.url.starts_with("/") {
                format!("{}{}", self.base_url, chapter.url)
            } else {
                chapter.url.clone()
            };
            
            match self.fetch_chapter_content(
                &full_chapter_url,
                volume_index,
                chapter_index,
                volume_title,
                &chapter.title,
                images_dir,
                chapter.has_illustrations,
            ).await {
                Ok(xhtml_path) => {
                    chapter.xhtml_path = Some(xhtml_path);
                    println!("  章节 '{}': 已处理", chapter.title);
                },
                Err(e) => {
                    println!("  章节 '{}' 处理失败: {}", chapter.title, e);
                    // 继续处理其他章节
                }
            }
            
            // 添加短暂延迟，避免请求过快
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        
        Ok(())
    }

    async fn download_cover_image(&self, image_url: &str, _novel_id: u32, _title: &str, epub_dir: &Path) -> Result<Option<String>> {
        // 从URL中提取文件扩展名
        let extension = Path::new(image_url)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("jpg");
        
        // EPUB标准目录结构: OEBPS/images/
        let images_dir = epub_dir.join("OEBPS").join("images");
        fs::create_dir_all(&images_dir)?;
        
        // 小说封面命名为cover
        let filename = format!("cover.{}", extension);
        
        // 使用通用函数下载封面图片
        self.download_cover_image_common(image_url, &images_dir, &filename, "小说", true).await
    }

    pub async fn parse_novel_info(&self, html_content: &str, url: &str, novel_id: u32) -> Result<NovelInfo> {
        let document = Html::parse_document(html_content);
        
        // 解析小说标题
        let title_selector = Selector::parse("span.series-name > a").unwrap();
        let title = document
            .select(&title_selector)
            .next()
            .ok_or_else(|| anyhow::anyhow!("未找到小说标题"))?
            .text()
            .collect::<String>()
            .trim()
            .to_string();

        // 解析作者和插画师信息
        let mut author = String::new();
        let mut illustrator = None;
        let info_item_selector = Selector::parse("div.info-item").unwrap();
        let info_name_selector = Selector::parse("span.info-name").unwrap();
        let info_value_selector = Selector::parse("span.info-value > a").unwrap();
        
        for info_item in document.select(&info_item_selector) {
            if let Some(info_name) = info_item.select(&info_name_selector).next() {
                let info_name_text = info_name.text().collect::<String>();
                
                if info_name_text.contains("Tác giả:") {
                    // 解析作者
                    if let Some(author_link) = info_item.select(&info_value_selector).next() {
                        author = author_link.text().collect::<String>().trim().to_string();
                    }
                } else if info_name_text.contains("Họa sĩ:") {
                    // 解析插画师
                    if let Some(illustrator_link) = info_item.select(&info_value_selector).next() {
                        let illustrator_text = illustrator_link.text().collect::<String>().trim().to_string();
                        if !illustrator_text.is_empty() {
                            illustrator = Some(illustrator_text);
                        }
                    }
                }
            }
        }

        if author.is_empty() {
            return Err(anyhow::anyhow!("未找到作者信息"));
        }

        // 解析简介内容
        let mut summary = String::new();
        let summary_selector = Selector::parse("div.summary-content > p").unwrap();
        let summary_paragraphs: Vec<String> = document
            .select(&summary_selector)
            .map(|p| p.text().collect::<String>().trim().to_string())
            .filter(|text| !text.is_empty())
            .collect();
        
        if !summary_paragraphs.is_empty() {
            summary = summary_paragraphs.join("\n");
        }

        // 创建EPUB标准目录结构
        let epub_dir_name = format!("epub_{}", novel_id);
        let epub_dir = Path::new(&epub_dir_name);
        
        // 解析封面图片URL并下载
        let mut cover_image_path = None;
        let cover_selector = Selector::parse("div.content.img-in-ratio").unwrap();
        if let Some(cover_div) = document.select(&cover_selector).next() {
            if let Some(style) = cover_div.value().attr("style") {
                // 从style属性中提取URL: background-image: url('...')
                if let Some(start) = style.find("url('") {
                    let start = start + 5; // 跳过 "url('"
                    if let Some(end) = style[start..].find("')") {
                        let image_url = &style[start..start + end];
                        // 下载封面图片
                        match self.download_cover_image(image_url, novel_id, &title, &epub_dir).await {
                            Ok(Some(path)) => cover_image_path = Some(path),
                            Ok(None) => {
                                // 默认封面图片，不下载
                                println!("使用默认封面图片，跳过下载");
                            },
                            Err(e) => println!("下载封面图片失败: {}", e),
                        }
                    }
                }
            }
        }

        // 解析卷信息
        let mut volumes = Vec::new();
        let list_vol_section_selector = Selector::parse("section#list-vol").unwrap();
        let list_volume_selector = Selector::parse("ol.list-volume").unwrap();
        let volume_item_selector = Selector::parse("li").unwrap();
        let volume_title_selector = Selector::parse("span.list_vol-title").unwrap();
        
        if let Some(list_vol_section) = document.select(&list_vol_section_selector).next() {
            if let Some(list_volume) = list_vol_section.select(&list_volume_selector).next() {
                for (volume_index, volume_item) in list_volume.select(&volume_item_selector).enumerate() {
                    // 获取卷标题
                    let volume_title = volume_item
                        .select(&volume_title_selector)
                        .next()
                        .map(|span| span.text().collect::<String>().trim().to_string())
                        .unwrap_or_else(|| "未知卷".to_string());
                    
                    // 获取卷的data-scrollto属性
                    let volume_id = volume_item
                        .value()
                        .attr("data-scrollto")
                        .unwrap_or("")
                        .to_string();
                    
                    if !volume_id.is_empty() {
                        let volume_element_id = volume_id.trim_start_matches('#');
                        
                        // 根据volume_id找到对应的卷元素并提取封面图片
                        let volume_header_selector = Selector::parse(&format!("header#{}", volume_element_id)).unwrap();
                        let volume_cover_selector = Selector::parse("div.volume-cover div.content.img-in-ratio").unwrap();
                        
                        let mut volume_cover_path = None;
                        
                        // 查找卷封面图片
                        if let Some(volume_header) = document.select(&volume_header_selector).next() {
                            if let Some(parent_element) = volume_header.parent_element() {
                                if let Some(cover_div) = parent_element.select(&volume_cover_selector).next() {
                                    if let Some(style) = cover_div.value().attr("style") {
                                        // 从style属性中提取URL: background-image: url('...')
                                        if let Some(start) = style.find("url('") {
                                            let start = start + 5; // 跳过 "url('"
                                            if let Some(end) = style[start..].find("')") {
                                                let image_url = &style[start..start + end];
                                                // 下载卷封面图片
                                                match self.download_volume_cover_image(image_url, volume_index, &volume_title, &epub_dir).await {
                                                    Ok(path) => volume_cover_path = path,
                                                    Err(e) => println!("下载卷 '{}' 封面图片失败: {}", volume_title, e),
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // 解析该卷的章节信息
                        let mut chapters = self.parse_volume_chapters(&document, &volume_id, novel_id, &volume_title);
                        
                        // 处理该卷的章节内容
                        if !chapters.is_empty() {
                            println!("\n正在处理卷 '{}' 的 {} 个章节...", volume_title, chapters.len());
                            
                            // 创建EPUB标准的images目录
                            let images_dir = epub_dir.join("OEBPS").join("images");
                            fs::create_dir_all(&images_dir)?;
                            
                            match self.fetch_and_process_chapters(
                                &mut chapters,
                                volume_index, // 使用正确的卷索引
                                &volume_title,
                                &title,
                                &images_dir,
                            ).await {
                                Ok(()) => println!("卷 '{}' 章节处理完成", volume_title),
                                Err(e) => println!("处理卷 '{}' 章节时出错: {}", volume_title, e),
                            }
                        }
                        
                        volumes.push(Volume {
                            title: volume_title,
                            volume_id: volume_id.clone(),
                            cover_image_path: volume_cover_path,
                            chapters,
                        });
                    }
                }
            }
        }

        // 解析标签
        let mut tags = Vec::new();
        let tags_selector = Selector::parse("div.series-gernes > a").unwrap();
        for tag_element in document.select(&tags_selector) {
            let tag_text = tag_element.text().collect::<String>().trim().to_string();
            if !tag_text.is_empty() {
                tags.push(tag_text);
            }
        }

        // 创建NovelInfo结构体
        let novel_info = NovelInfo {
            title,
            author,
            illustrator,
            summary,
            cover_image_path,
            volumes,
            tags,
            url: url.to_string(),
        };

        // 生成EPUB元数据文件
        let epub_generator = crate::epub::EpubGenerator::new();
        epub_generator.generate_epub_metadata(&novel_info, &epub_dir, novel_id).await?;

        // 压缩EPUB文件夹为EPUB文件
        match epub_generator.compress_epub(&epub_dir, &novel_info.title) {
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