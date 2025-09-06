use std::error::Error;
use std::fs;
use std::path::Path;
use crate::models::NovelInfo;

pub struct EpubGenerator;

impl EpubGenerator {
    pub fn new() -> Self {
        Self
    }

    pub async fn generate_epub_metadata(&self,
        novel_info: &NovelInfo,
        epub_dir: &Path,
        _novel_id: u32,
    ) -> Result<(), Box<dyn Error>> {
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
        <dc:identifier id="BookId">docln:"#);
        content_opf.push_str(&format!("{}", _novel_id));
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
        
        // 添加章节到spine - 按卷的顺序添加
        for (i, volume) in novel_info.volumes.iter().enumerate() {
            for (j, chapter) in volume.chapters.iter().enumerate() {
                if chapter.xhtml_path.is_some() {
                    content_opf.push_str(&format!(r#"
        <itemref idref="chapter{}_{}"/">"#, i + 1, j + 1));
                }
            }
        }
        
        content_opf.push_str(r#"
    </spine>
    <guide>"#);
        
        // 添加封面指南
        content_opf.push_str(r#"
        <reference type="cover" title="Cover" href="images/cover.jpg"/">"#);
        
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
        toc_ncx.push_str(&format!("docln:{}", _novel_id));
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
        
        // 添加章节导航 - 层级结构
        let mut nav_point_counter = 1;
        for volume in novel_info.volumes.iter() {
            let processed_chapters: Vec<&crate::models::Chapter> = volume.chapters.iter()
                .filter(|c| c.xhtml_path.is_some())
                .collect();
            
            if !processed_chapters.is_empty() {
                // 卷作为一级导航点
                toc_ncx.push_str(&format!(r#"
        <navPoint id="navPoint{}" playOrder="{}">
            <navLabel>
                <text>{}</text>
            </navLabel>
            <content src="{}"/>"#,
                    nav_point_counter, nav_point_counter, volume.title,
                    processed_chapters.first().unwrap().xhtml_path.as_ref().unwrap()));
                nav_point_counter += 1;
                
                // 章节作为卷的子导航点
                for chapter in processed_chapters {
                    if let Some(xhtml_path) = &chapter.xhtml_path {
                        toc_ncx.push_str(&format!(r#"
            <navPoint id="navPoint{}" playOrder="{}">
                <navLabel>
                    <text>{}</text>
                </navLabel>
                <content src="{}"/>
            </navPoint>"#,
                            nav_point_counter, nav_point_counter, chapter.title, xhtml_path));
                        nav_point_counter += 1;
                    }
                }
                
                toc_ncx.push_str(r#"
        </navPoint>"#);
            }
        }
        
        toc_ncx.push_str(r#"
    </navMap>
</ncx>"#);
        
        fs::write(oebps_dir.join("toc.ncx"), toc_ncx)?;
        
        println!("EPUB元数据文件已生成");
        Ok(())
    }
}