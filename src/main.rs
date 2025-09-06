use reqwest;
use scraper::{Html, Selector, Element};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{self, Write};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
struct Chapter {
    title: String,
    url: String,
    has_illustrations: bool, // 是否包含插图
    xhtml_path: Option<String>, // XHTML文件路径（用于EPUB）
    illustration_paths: Vec<String>, // 本地插图路径
}

#[derive(Debug, Serialize, Deserialize)]
struct Volume {
    title: String,
    volume_id: String,
    cover_image_path: Option<String>,
    chapters: Vec<Chapter>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NovelInfo {
    title: String,
    author: String,
    illustrator: Option<String>, // 插画师
    summary: String, // 简介内容，多段合并为一个String
    cover_image_path: Option<String>, // 封面图片本地路径
    volumes: Vec<Volume>, // 卷信息
    tags: Vec<String>,
    url: String,
}

#[derive(Debug)]
enum NovelCategory {
    SangTac ,// 原创
    AiDich,  // AI翻译
}

impl NovelCategory {
    fn to_url_path(&self) -> &str {
        match self {
            NovelCategory::SangTac => "sang-tac",
            NovelCategory::AiDich => "ai-dich",
        }
    }
}

struct DoclnCrawler {
    client: reqwest::Client,
    base_url: String,
}

impl DoclnCrawler {
    fn new() -> Self {
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

    async fn fetch_novel_info(&self, category: NovelCategory, novel_id: u32) -> Result<NovelInfo, Box<dyn Error>> {
        let url = format!("{}/{}/{}", self.base_url, category.to_url_path(), novel_id);
        
        println!("正在获取: {}", url);
        
        let response = self.client.get(&url).send().await?;
        let html_content = response.text().await?;
        
        self.parse_novel_info(&html_content, &url, novel_id).await
    }

    async fn download_volume_cover_image(&self, image_url: &str, _volume_index: usize, volume_title: &str, epub_dir: &Path) -> Result<Option<String>, Box<dyn Error>> {
        // 检查是否为默认的nocover图片
        if image_url.contains("nocover") {
            println!("卷 '{}' 使用默认封面图片，跳过下载", volume_title);
            return Ok(None);
        }
        
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
        let filepath = images_dir.join(&filename);
        
        println!("正在下载卷 '{}' 的封面图片: {}", volume_title, image_url);
        
        // 下载图片
        let response = self.client.get(image_url).send().await?;
        let image_bytes = response.bytes().await?;
        
        // 保存到本地
        fs::write(&filepath, &image_bytes)?;
        
        println!("卷 '{}' 的封面图片已保存到: {} (文件名: {})", volume_title, filepath.display(), filename);
        
        // 返回相对路径（相对于OEBPS目录）
        Ok(Some(format!("images/{}", filename)))
    }

    async fn download_chapter_illustrations(
        &self,
        chapter_paragraphs: &[String],
        images_dir: &Path,
        chapter_index: usize,
        volume_index: usize,
        _volume_title: &str,
        _chapter_title: &str,
    ) -> Result<(String, Vec<String>), Box<dyn Error>> {
        let mut illustration_paths = Vec::new();
        let mut modified_paragraphs = Vec::new();
        let mut illustration_counter = 1;
        let mut has_any_images = false;
        
        // 首先检查是否有任何图片
        for p_html in chapter_paragraphs {
            let p_document = Html::parse_fragment(&p_html);
            let img_selector = Selector::parse("img").unwrap();
            if p_document.select(&img_selector).count() > 0 {
                has_any_images = true;
                break;
            }
        }
        
        // 只有在有图片时才创建插图目录 - 按卷文件夹组织
        let illustrations_dir = if has_any_images {
            let volume_img_dir = images_dir.join(format!("volume_{:03}", volume_index + 1));
            let chapter_img_dir = volume_img_dir.join(format!("chapter_{:03}", chapter_index + 1));
            fs::create_dir_all(&chapter_img_dir)?;
            Some(chapter_img_dir)
        } else {
            None
        };
        
        // 处理每个段落
        for p_html in chapter_paragraphs {
            let mut modified_p_html = p_html.clone();
            
            // 解析段落HTML来查找图片
            let p_document = Html::parse_fragment(&p_html);
            let img_selector = Selector::parse("img").unwrap();
            
            // 检查段落中是否有图片
            let has_images = p_document.select(&img_selector).count() > 0;
            
            if has_images {
                // 只有在有图片且有插图目录时才处理
                if let Some(ref illustrations_dir) = illustrations_dir {
                    // 处理段落中的图片
                    for img_element in p_document.select(&img_selector) {
                        if let Some(img_src) = img_element.value().attr("src") {
                            if !img_src.is_empty() {
                                // 下载图片
                                match self.download_illustration(img_src, illustrations_dir, illustration_counter, volume_index, chapter_index).await {
                                    Ok(local_path) => {
                                        // 替换原始src为本地路径（相对于images目录）
                                        let original_img_html = img_element.html();
                                        let filename = format!("{:03}.{}", illustration_counter, 
                                            Path::new(img_src).extension().and_then(|e| e.to_str()).unwrap_or("jpg"));
                                        let modified_img_html = original_img_html.replace(img_src, &format!("volume_{:03}/chapter_{:03}/{}", volume_index + 1, chapter_index + 1, filename));
                                        modified_p_html = modified_p_html.replace(&original_img_html, &modified_img_html);
                                        
                                        illustration_paths.push(local_path);
                                        illustration_counter += 1;
                                    },
                                    Err(e) => {
                                        println!("下载插图失败: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            modified_paragraphs.push(modified_p_html);
        }
        
        Ok((modified_paragraphs.join("\n"), illustration_paths))
    }

    async fn download_illustration(
        &self,
        image_url: &str,
        illustrations_dir: &Path,
        illustration_number: usize,
        volume_index: usize,
        chapter_index: usize,
    ) -> Result<String, Box<dyn Error>> {
        // 从URL中提取文件扩展名
        let extension = Path::new(image_url)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("jpg");
        
        // 插图命名为顺序编号
        let filename = format!("{:03}.{}", illustration_number, extension);
        let filepath = illustrations_dir.join(&filename);
        
        println!("正在下载插图 {}: {}", illustration_number, image_url);
        
        // 下载图片
        let response = self.client.get(image_url).send().await?;
        let image_bytes = response.bytes().await?;
        
        // 保存到本地
        fs::write(&filepath, &image_bytes)?;
        
        println!("插图 {} 已保存到: {}", illustration_number, filepath.display());
        
        // 返回相对路径（相对于images目录）
        Ok(format!("volume_{:03}/chapter_{:03}/{}", volume_index + 1, chapter_index + 1, filename))
    }

    async fn fetch_chapter_content(
        &self,
        chapter_url: &str,
        volume_index: usize,
        chapter_index: usize,
        volume_title: &str,
        chapter_title: &str,
        images_dir: &Path,
    ) -> Result<(String, Vec<String>), Box<dyn Error>> {
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
        
        // 下载插图并替换路径
        let (modified_content, illustration_paths) = self.download_chapter_illustrations(
            &chapter_paragraphs,
            images_dir,
            chapter_index,
            volume_index,
            volume_title,
            chapter_title,
        ).await?;
        
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
        Ok((format!("text/volume_{:03}/{}", volume_index + 1, xhtml_filename), illustration_paths))
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
                                        illustration_paths: Vec::new(),
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

    async fn fetch_and_process_chapters(
        &self,
        chapters: &mut Vec<Chapter>,
        volume_index: usize,
        volume_title: &str,
        _novel_title: &str,
        images_dir: &Path,
    ) -> Result<(), Box<dyn Error>> {
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
            ).await {
                Ok((xhtml_path, illustration_paths)) => {
                    chapter.xhtml_path = Some(xhtml_path);
                    chapter.illustration_paths = illustration_paths;
                    
                    if !chapter.illustration_paths.is_empty() {
                        chapter.has_illustrations = true;
                        println!("  章节 '{}': 已处理，包含 {} 张插图", chapter.title, chapter.illustration_paths.len());
                    } else {
                        println!("  章节 '{}': 已处理", chapter.title);
                    }
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

    async fn generate_epub_metadata(&self, 
        novel_info: &NovelInfo, 
        epub_dir: &Path,
        _novel_id: u32,
    ) -> Result<(), Box<dyn Error>> {
        use std::fs;
        
        // 创建EPUB标准目录
        let meta_inf_dir = epub_dir.join("META-INF");
        fs::create_dir_all(&meta_inf_dir)?;
        
        let oebps_dir = epub_dir.join("OEBPS");
        fs::create_dir_all(&oebps_dir)?;
        
        // 1. 生成 mimetype 文件
        let mimetype_content = "application/epub+zip";
        fs::write(epub_dir.join("mimetype"), mimetype_content)?;
        
        // 2. 生成 META-INF/container.xml
        let container_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
    <rootfiles>
        <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
    </rootfiles>
</container>"#;
        fs::write(meta_inf_dir.join("container.xml"), container_content)?;
        
        // 3. 生成 OEBPS/content.opf
        let mut content_opf = String::new();
        
        // OPF头部
        content_opf.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>
<package version="2.0" xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId">
    <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
        <dc:identifier id="BookId">urn:uuid:"#);
        content_opf.push_str(&format!("{}", uuid::Uuid::new_v4()));
        content_opf.push_str(r#"</dc:identifier>
        <dc:title>"#);
        content_opf.push_str(&novel_info.title);
        content_opf.push_str(r#"</dc:title>
        <dc:language>vi</dc:language>
        <dc:creator opf:role="aut">"#);
        content_opf.push_str(&novel_info.author);
        content_opf.push_str(r#"</dc:creator>"#);
        
        // 添加插画师信息
        if let Some(illustrator) = &novel_info.illustrator {
            content_opf.push_str(r#"
        <dc:contributor opf:role="ill">"#);
            content_opf.push_str(illustrator);
            content_opf.push_str(r#"</dc:contributor>"#);
        }
        
        // 添加标签
        for tag in &novel_info.tags {
            content_opf.push_str(r#"
        <dc:subject>"#);
            content_opf.push_str(tag);
            content_opf.push_str(r#"</dc:subject>"#);
        }
        
        // 添加简介
        if !novel_info.summary.is_empty() {
            content_opf.push_str(r#"
        <dc:description>"#);
            content_opf.push_str(&novel_info.summary);
            content_opf.push_str(r#"</dc:description>"#);
        }
        
        content_opf.push_str(r#"
        <dc:publisher>docln-fetch</dc:publisher>
        <dc:date>"#);
        content_opf.push_str(&chrono::Local::now().format("%Y-%m-%d").to_string());
        content_opf.push_str(r#"</dc:date>
        <meta name="generator" content="docln-fetch"/>
    </metadata>
    <manifest>"#);
        
        // manifest内容
        content_opf.push_str(r#"
        <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
        <item id="cover-image" href="images/cover.jpg" media-type="image/jpeg"/>"#);
        
        // 添加卷封面图片
        for (i, volume) in novel_info.volumes.iter().enumerate() {
            if let Some(cover_path) = &volume.cover_image_path {
                if let Some(filename) = Path::new(cover_path).file_name() {
                    if let Some(filename_str) = filename.to_str() {
                        let media_type = if filename_str.ends_with(".png") { "image/png" } else { "image/jpeg" };
                        content_opf.push_str(&format!(r#"
        <item id="volume{}-cover" href="images/{}" media-type="{}"/>"#, i + 1, filename_str, media_type));
                    }
                }
            }
        }
        
        // 添加章节文件
        for (i, volume) in novel_info.volumes.iter().enumerate() {
            for (j, chapter) in volume.chapters.iter().enumerate() {
                if let Some(xhtml_path) = &chapter.xhtml_path {
                    if let Some(filename) = Path::new(xhtml_path).file_name() {
                        if let Some(_filename_str) = filename.to_str() {
                            content_opf.push_str(&format!(r#"
        <item id="chapter{}_{}" href="{}" media-type="application/xhtml+xml"/>"#, 
                                i + 1, j + 1, xhtml_path));
                        }
                    }
                }
            }
        }
        
        // spine内容
        content_opf.push_str(r#"
    </manifest>
    <spine toc="ncx">"#);
        
        // 添加章节到spine
        for (i, volume) in novel_info.volumes.iter().enumerate() {
            for (j, chapter) in volume.chapters.iter().enumerate() {
                if chapter.xhtml_path.is_some() {
                    content_opf.push_str(&format!(r#"
        <itemref idref="chapter{}_{}"/>"#, i + 1, j + 1));
                }
            }
        }
        
        content_opf.push_str(r#"
    </spine>
    <guide>"#);
        
        // 添加封面指南
        content_opf.push_str(r#"
        <reference type="cover" title="Cover" href="images/cover.jpg"/>"#);
        
        content_opf.push_str(r#"
    </guide>
</package>"#);
        
        fs::write(oebps_dir.join("content.opf"), content_opf)?;
        
        // 4. 生成 OEBPS/toc.ncx
        let mut toc_ncx = String::new();
        
        toc_ncx.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>
<ncx version="2005-1" xmlns="http://www.daisy.org/z3986/2005/ncx/">
    <head>
        <meta name="dtb:uid" content=""#);
        toc_ncx.push_str(&format!("{}", uuid::Uuid::new_v4()));
        toc_ncx.push_str(r#""/>
        <meta name="dtb:depth" content="1"/>
        <meta name="dtb:totalPageCount" content="0"/>
        <meta name="dtb:maxPageNumber" content="0"/>
    </head>
    <docTitle>
        <text>"#);
        toc_ncx.push_str(&novel_info.title);
        toc_ncx.push_str(r#"</text>
    </docTitle>
    <navMap>"#);
        
        // 添加章节导航
        let mut nav_point_counter = 1;
        for (_i, volume) in novel_info.volumes.iter().enumerate() {
            for (_j, chapter) in volume.chapters.iter().enumerate() {
                if let Some(xhtml_path) = &chapter.xhtml_path {
                    toc_ncx.push_str(&format!(r#"
        <navPoint id="navPoint{}" playOrder="{}">
            <navLabel>
                <text>{} - {}</text>
            </navLabel>
            <content src="{}"/>
        </navPoint>"#,
                        nav_point_counter, nav_point_counter, volume.title, chapter.title, xhtml_path));
                    nav_point_counter += 1;
                }
            }
        }
        
        toc_ncx.push_str(r#"
    </navMap>
</ncx>"#);
        
        fs::write(oebps_dir.join("toc.ncx"), toc_ncx)?;
        
        println!("EPUB元数据文件已生成");
        Ok(())
    }

    fn generate_directory_structure(novel_info: &NovelInfo, _novel_id: u32) {
        use std::path::Path;
        let epub_dir = format!("epub_{}", _novel_id);
        
        println!("\n=== EPUB目录结构 ===");
        println!("{}/", epub_dir);
        println!("├── mimetype                    # MIME类型文件");
        println!("├── META-INF/");
        println!("│   └── container.xml          # 容器文件");
        println!("└── OEBPS/                     # OEBPS目录");
        println!("    ├── content.opf            # OPF元数据文件");
        println!("    ├── toc.ncx                # NCX导航文件");
        println!("    ├── images/                # 图片资源");
        
        if let Some(_cover_path) = &novel_info.cover_image_path {
            println!("    │   └── cover.jpg          # 小说封面");
        }
        
        // 卷封面
        if !novel_info.volumes.is_empty() {
            for (i, volume) in novel_info.volumes.iter().enumerate() {
                if let Some(cover_path) = &volume.cover_image_path {
                    if let Some(filename) = Path::new(cover_path).file_name() {
                        if let Some(filename_str) = filename.to_str() {
                            println!("    │   └── {} (卷 {} 封面)", filename_str, i + 1);
                        }
                    }
                }
            }
        }
        
        println!("    ├── images/                # 图片资源");
        
        if let Some(_cover_path) = &novel_info.cover_image_path {
            println!("    │   └── cover.jpg          # 小说封面");
        }
        
        // 卷封面和图片目录
        if !novel_info.volumes.is_empty() {
            for (i, volume) in novel_info.volumes.iter().enumerate() {
                let mut has_content = false;
                
                // 显示卷封面
                if let Some(cover_path) = &volume.cover_image_path {
                    if let Some(filename) = Path::new(cover_path).file_name() {
                        if let Some(filename_str) = filename.to_str() {
                            println!("    │   ├── volume_{:03}/       # 卷 {} 图片目录", i + 1, i + 1);
                            println!("    │   │   └── {} (卷 {} 封面)", filename_str, i + 1);
                            has_content = true;
                        }
                    }
                } else {
                    println!("    │   ├── volume_{:03}/       # 卷 {} 图片目录", i + 1, i + 1);
                }
                
                // 显示章节图片文件夹
                for (j, chapter) in volume.chapters.iter().enumerate() {
                    if !chapter.illustration_paths.is_empty() {
                        if !has_content {
                            println!("    │   │   ├── chapter_{:03}/  # {}张插图", j + 1, chapter.illustration_paths.len());
                            has_content = true;
                        } else {
                            println!("    │   │   ├── chapter_{:03}/  # {}张插图", j + 1, chapter.illustration_paths.len());
                        }
                    }
                }
                
                if i < novel_info.volumes.len() - 1 {
                    if has_content {
                        println!("    │   │");
                    }
                }
            }
        }
        
        println!("    └── text/                  # XHTML文本内容");
        
        // 章节文件 - 按卷文件夹组织
        if !novel_info.volumes.is_empty() {
            for (i, volume) in novel_info.volumes.iter().enumerate() {
                if !volume.chapters.is_empty() {
                    let processed_chapters: Vec<&Chapter> = volume.chapters.iter()
                        .filter(|c| c.xhtml_path.is_some())
                        .collect();
                    
                    if !processed_chapters.is_empty() {
                        println!("        ├── volume_{:03}/          # 卷 {} - {}", i + 1, i + 1, volume.title);
                        
                        let display_count = std::cmp::min(3, processed_chapters.len());
                        for (_j, chapter) in processed_chapters.iter().take(display_count).enumerate() {
                            let chapter_prefix = if !chapter.illustration_paths.is_empty() { "📄" } else { "📖" };
                            if let Some(xhtml_path) = &chapter.xhtml_path {
                                if let Some(filename) = Path::new(xhtml_path).file_name() {
                                    if let Some(filename_str) = filename.to_str() {
                                        println!("        │   ├── {} {}", chapter_prefix, filename_str);
                                    }
                                }
                            }
                        }
                        
                        if processed_chapters.len() > display_count {
                            let remaining = processed_chapters.len() - display_count;
                            println!("        │   └── ... (还有 {} 个章节)", remaining);
                        }
                        
                        if i < novel_info.volumes.len() - 1 {
                            println!("        │");
                        }
                    }
                }
            }
        }
        
        println!("\n📁 EPUB结构说明:");
        println!("  📄 表示包含插图的章节");
        println!("  📖 表示普通章节");
        println!("  所有文件都符合EPUB 3.0标准");
        println!("  图片按卷保存在OEBPS/images/volume_XXX/目录下");
        println!("  文本内容按卷保存在OEBPS/text/volume_XXX/目录下");
        println!("  可直接使用EPUB工具打包生成.epub文件");
    }

    async fn download_cover_image(&self, image_url: &str, _novel_id: u32, _title: &str, epub_dir: &Path) -> Result<Option<String>, Box<dyn Error>> {
        // 检查是否为默认的nocover图片
        if image_url.contains("nocover") {
            println!("检测到默认封面图片，跳过下载");
            return Ok(None);
        }
        
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
        let filepath = images_dir.join(&filename);
        
        println!("正在下载封面图片: {}", image_url);
        
        // 下载图片
        let response = self.client.get(image_url).send().await?;
        let image_bytes = response.bytes().await?;
        
        // 保存到本地
        fs::write(&filepath, &image_bytes)?;
        
        println!("封面图片已保存到: {}", filepath.display());
        
        // 返回相对路径（相对于OEBPS目录）
        Ok(Some(format!("images/{}", filename)))
    }

    async fn parse_novel_info(&self, html_content: &str, url: &str, novel_id: u32) -> Result<NovelInfo, Box<dyn Error>> {
        let document = Html::parse_document(html_content);
        
        // 解析小说标题
        let title_selector = Selector::parse("span.series-name > a").unwrap();
        let title = document
            .select(&title_selector)
            .next()
            .ok_or("未找到小说标题")?
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
            return Err("未找到作者信息".into());
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
                                volumes.len(), // 使用当前卷数量作为索引
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
        self.generate_epub_metadata(&novel_info, &epub_dir, novel_id).await?;

        Ok(novel_info)
    }

    async fn crawl_novel(&self, category: NovelCategory, novel_id: u32) {
        match self.fetch_novel_info(category, novel_id).await {
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
                    println!("封面已下载: {}", cover_path);
                } else {
                    println!("封面: 使用默认封面");
                }
                println!("标签: {}", novel_info.tags.join(", "));
                
                // 显示卷信息
                if !novel_info.volumes.is_empty() {
                    println!("\n卷信息 (共 {} 卷):", novel_info.volumes.len());
                    for (i, volume) in novel_info.volumes.iter().enumerate() {
                        println!("  卷 {}: {}", i + 1, volume.title);
                        if let Some(cover_path) = &volume.cover_image_path {
                            println!("    卷封面已下载: {}", cover_path);
                        } else {
                            println!("    卷封面: 使用默认封面");
                        }
                        // 显示章节信息
                        if !volume.chapters.is_empty() {
                            println!("    章节数量: {}", volume.chapters.len());
                            // 显示包含插图的章节数量
                            let illustration_count = volume.chapters.iter().filter(|c| c.has_illustrations).count();
                            if illustration_count > 0 {
                                println!("    含插图章节: {} 章", illustration_count);
                            }
                            
                            // 显示已处理的章节数量
                            let processed_count = volume.chapters.iter().filter(|c| c.xhtml_path.is_some()).count();
                            if processed_count > 0 {
                                println!("    已处理章节: {} 章", processed_count);
                            }
                        }
                    }
                }
                
                println!("URL: {}", novel_info.url);
                println!("==============\n");
                
                // 显示建议的目录结构
                Self::generate_directory_structure(&novel_info, novel_id);
            }
            Err(e) => {
                println!("爬取小说失败 (ID: {}): {}", novel_id, e);
            }
        }
    }
}


fn get_user_input() -> Result<(NovelCategory, u32), Box<dyn Error>> {
    println!("请选择小说分区:");
    println!("1. 原创区 (sang-tac)");
    println!("2. AI翻译区 (ai-dich)");
    println!("请输入选择 (1 或 2): ");
    
    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;
    let choice = choice.trim();
    
    let category = match choice {
        "1" => NovelCategory::SangTac,
        "2" => NovelCategory::AiDich,
        _ => return Err("无效的选择，请输入 1 或 2".into()),
    };
    
    println!("请输入小说ID: ");
    let mut novel_id = String::new();
    io::stdin().read_line(&mut novel_id)?;
    let novel_id: u32 = novel_id.trim().parse()
        .map_err(|_| "请输入有效的小说ID (数字)")?;
    
    Ok((category, novel_id))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let crawler = DoclnCrawler::new();
    
    loop {
        println!("\n=== docln-fetch ===");
        match get_user_input() {
            Ok((category, novel_id)) => {
                let category_name = match category {
                    NovelCategory::SangTac => "原创区",
                    NovelCategory::AiDich => "AI翻译区",
                };
                println!("\n正在爬取{} ID为 {} 的小说...", category_name, novel_id);
                crawler.crawl_novel(category, novel_id).await;
            }
            Err(e) => {
                println!("输入错误: {}", e);
            }
        }
        
        print!("\n是否继续爬取其他小说? (y/n): ");
        io::stdout().flush()?;
        let mut continue_choice = String::new();
        io::stdin().read_line(&mut continue_choice)?;
        if continue_choice.trim().to_lowercase() != "y" {
            break;
        }
    }
    
    println!("程序结束。");
    Ok(())
}