use anyhow::Result;
use std::fs;
use std::path::Path;
use super::Epub;

pub struct MetadataGenerator;

impl MetadataGenerator {
    pub fn new() -> Self {
        Self
    }

    /// 生成mimetype文件
    pub fn generate_mimetype(&self, epub_dir: &Path) -> Result<()> {
        let mimetype_content = "application/epub+zip";
        fs::write(epub_dir.join("mimetype"), mimetype_content)?;
        Ok(())
    }

    /// 生成container.xml文件
    pub fn generate_container_xml(&self, meta_inf_dir: &Path) -> Result<()> {
        let container_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
    <rootfiles>
        <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
    </rootfiles>
</container>"#;
        fs::write(meta_inf_dir.join("container.xml"), container_content)?;
        Ok(())
    }

    /// 生成content.opf文件
    pub fn generate_content_opf(&self, epub: &Epub, oebps_dir: &Path, novel_id: u32) -> Result<()> {
        let mut content_opf = String::new();
        
        // OPF头部
        content_opf.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>
<package version="2.0" xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId">
    <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
        <dc:identifier id="BookId">docln:"#);
        content_opf.push_str(&format!("{}", novel_id));
        content_opf.push_str(r#"</dc:identifier>
        <dc:title>"#);
        content_opf.push_str(&epub.title);
        content_opf.push_str(r#"</dc:title>
        <dc:language>vi</dc:language>
        <dc:creator opf:role="aut">"#);
        content_opf.push_str(&epub.author);
        content_opf.push_str(r#"</dc:creator>"#);
        
        // 添加插画师信息
        if let Some(illustrator) = &epub.illustrator {
            content_opf.push_str(r#"
        <dc:contributor opf:role="ill">"#);
            content_opf.push_str(illustrator);
            content_opf.push_str(r#"</dc:contributor>"#);
        }
        
        // 添加标签
        for tag in &epub.tags {
            content_opf.push_str(r#"
        <dc:subject>"#);
            content_opf.push_str(tag);
            content_opf.push_str(r#"</dc:subject>"#);
        }
        
        // 添加简介
        if !epub.summary.is_empty() {
            content_opf.push_str(r#"
        <dc:description>"#);
            content_opf.push_str(&epub.summary);
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
        for (i, volume) in epub.volumes.iter().enumerate() {
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
        
        // 添加章节插图图片
        for (i, volume) in epub.volumes.iter().enumerate() {
            for (j, chapter) in volume.chapters.iter().enumerate() {
                if chapter.has_illustrations {
                    // 为每个有插图的章节添加图片文件声明
                    let volume_img_dir = oebps_dir.join("images").join(format!("volume_{:03}", i + 1));
                    let chapter_img_dir = volume_img_dir.join(format!("chapter_{:03}", j + 1));
                    
                    if chapter_img_dir.exists() {
                        if let Ok(entries) = std::fs::read_dir(&chapter_img_dir) {
                            for entry in entries.flatten() {
                                if let Ok(file_type) = entry.file_type() {
                                    if file_type.is_file() {
                                        if let Some(file_name) = entry.file_name().to_str() {
                                            if file_name.ends_with(".jpeg") || file_name.ends_with(".jpg") || file_name.ends_with(".png") {
                                                let media_type = if file_name.ends_with(".png") { "image/png" } else { "image/jpeg" };
                                                let img_path = format!("images/volume_{:03}/chapter_{:03}/{}", i + 1, j + 1, file_name);
                                                let img_id = format!("vol{}_chap{}_img{}", i + 1, j + 1, file_name);
                                                content_opf.push_str(&format!(r#"
        <item id="{}" href="{}" media-type="{}"/>"#, img_id, img_path, media_type));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // 添加章节文件
        for (i, volume) in epub.volumes.iter().enumerate() {
            // 为有卷封面的卷添加章节0
            if volume.cover_image_path.is_some() {
                let chapter0_path = format!("text/volume_{:03}/chapter_000.xhtml", i + 1);
                content_opf.push_str(&format!(r#"
        <item id="chapter{}_0" href="{}" media-type="application/xhtml+xml"/>"#, 
                                    i + 1, chapter0_path));
            }
            
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
        for (i, volume) in epub.volumes.iter().enumerate() {
            // 为有卷封面的卷添加章节0到spine
            if volume.cover_image_path.is_some() {
                content_opf.push_str(&format!(r#"
        <itemref idref="chapter{}_0"/>"#, i + 1));
            }
            
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
        Ok(())
    }

    /// 生成toc.ncx文件
    pub fn generate_toc_ncx(&self, epub: &Epub, oebps_dir: &Path, novel_id: u32) -> Result<()> {
        let mut toc_ncx = String::new();
        
        toc_ncx.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>
<ncx version="2005-1" xmlns="http://www.daisy.org/z3986/2005/ncx/">
    <head>
        <meta name="dtb:uid" content=""#);
        toc_ncx.push_str(&format!("docln:{}", novel_id));
        toc_ncx.push_str(r#""/>
        <meta name="dtb:depth" content="1"/>
        <meta name="dtb:totalPageCount" content="0"/>
        <meta name="dtb:maxPageNumber" content="0"/>
    </head>
    <docTitle>
        <text>"#);
        toc_ncx.push_str(&epub.title);
        toc_ncx.push_str(r#"</text>
    </docTitle>
    <navMap>"#);
        
        // 添加章节导航 - 层级结构
        let mut nav_point_counter = 1;
        for (volume_index, volume) in epub.volumes.iter().enumerate() {
            let processed_chapters: Vec<&crate::epub::chapter::Chapter> = volume.chapters.iter()
                .filter(|c| c.xhtml_path.is_some())
                .collect();
            
            if !processed_chapters.is_empty() {
                // 确定卷的指向目标：如果有卷封面则指向章节0，否则指向第一个章节
                let volume_target = if volume.cover_image_path.is_some() {
                    format!("text/volume_{:03}/chapter_000.xhtml", volume_index + 1)
                } else {
                    processed_chapters.first().unwrap().xhtml_path.as_ref().unwrap().clone()
                };
                
                // 卷作为一级导航点
                toc_ncx.push_str(&format!(r#"
        <navPoint id="navPoint{}" playOrder="{}">
            <navLabel>
                <text>{}</text>
            </navLabel>
            <content src="{}"/>"#,
                    nav_point_counter, nav_point_counter, volume.title, volume_target));
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
        Ok(())
    }

    /// 生成所有元数据文件
    pub fn generate_all_metadata(&self, epub: &Epub, epub_dir: &Path, novel_id: u32) -> Result<()> {
        // 创建EPUB标准目录
        let meta_inf_dir = epub_dir.join("META-INF");
        fs::create_dir_all(&meta_inf_dir)?;
        
        let oebps_dir = epub_dir.join("OEBPS");
        fs::create_dir_all(&oebps_dir)?;
        
        // 生成所有元数据文件
        self.generate_mimetype(epub_dir)?;
        self.generate_container_xml(&meta_inf_dir)?;
        self.generate_content_opf(epub, &oebps_dir, novel_id)?;
        self.generate_toc_ncx(epub, &oebps_dir, novel_id)?;
        
        println!("EPUB元数据文件已生成");
        Ok(())
    }
}