use anyhow::Result;
use std::fs;
use std::path::Path;
use std::fs::File;
use std::io::Write;
use zip::write::FileOptions;
use zip::ZipWriter;
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
    ) -> Result<()> {
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
        
        // 添加章节插图图片
        for (i, volume) in novel_info.volumes.iter().enumerate() {
            for (j, chapter) in volume.chapters.iter().enumerate() {
                if chapter.has_illustrations {
                    // 为每个有插图的章节添加图片文件声明
                    // 由于图片文件是在下载时动态创建的，我们需要扫描目录
                    let volume_img_dir = epub_dir.join("OEBPS").join("images").join(format!("volume_{:03}", i + 1));
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
        for (i, volume) in novel_info.volumes.iter().enumerate() {
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
        for (i, volume) in novel_info.volumes.iter().enumerate() {
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
        for (volume_index, volume) in novel_info.volumes.iter().enumerate() {
            let processed_chapters: Vec<&crate::models::Chapter> = volume.chapters.iter()
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
        
        // 为有卷封面的卷生成章节0 XHTML文件
        for (i, volume) in novel_info.volumes.iter().enumerate() {
            if volume.cover_image_path.is_some() {
                self.generate_volume_cover_chapter(volume, i, &oebps_dir)?;
            }
        }
        
        println!("EPUB元数据文件已生成");
        Ok(())
    }
    
    /// 生成卷封面章节0的XHTML文件
    fn generate_volume_cover_chapter(&self, volume: &crate::models::Volume, volume_index: usize, oebps_dir: &Path) -> Result<()> {
        let volume_dir = oebps_dir.join("text").join(format!("volume_{:03}", volume_index + 1));
        fs::create_dir_all(&volume_dir)?;
        
        let xhtml_filename = "chapter_000.xhtml";
        let xhtml_path = volume_dir.join(xhtml_filename);
        
        // 创建XHTML内容
        let mut xhtml_content = String::new();
        
        // XHTML头部
        xhtml_content.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
    <title>"#);
        xhtml_content.push_str(&volume.title);
        xhtml_content.push_str(r#"</title>
    <meta http-equiv="Content-Type" content="text/html; charset=UTF-8"/>
</head>
<body>
    <h1>"#);
        xhtml_content.push_str(&volume.title);
        xhtml_content.push_str(r#"</h1>
    <div class="volume-cover">
"#);
        
        // 添加卷封面图片
        if let Some(cover_path) = &volume.cover_image_path {
            // 从cover_path中提取文件名
            if let Some(filename) = Path::new(cover_path).file_name() {
                if let Some(filename_str) = filename.to_str() {
                    xhtml_content.push_str(&format!(r#"
        <img src="../../images/{}" alt="{}" style="max-width: 100%; height: auto;"/>"#, 
                            filename_str, volume.title));
                }
            }
        }
        
        // XHTML尾部
        xhtml_content.push_str(r#"
    </div>
</body>
</html>"#);
        
        fs::write(&xhtml_path, xhtml_content)?;
        
        println!("卷封面章节0已生成: {}", xhtml_path.display());
        Ok(())
    }

    /// 压缩EPUB文件夹为EPUB文件
    pub fn compress_epub(&self, epub_dir: &Path, _novel_title: &str) -> Result<String> {
        // 从目录名提取ID，目录名格式为 epub_{id}，转换为 docln_{id}
        let dir_name = epub_dir.file_name().unwrap().to_string_lossy();
        let epub_filename = if dir_name.starts_with("epub_") {
            format!("docln_{}.epub", &dir_name[5..])
        } else {
            format!("docln_{}.epub", &dir_name)
        };
        let epub_path = epub_dir.parent().unwrap().join(&epub_filename);
        
        println!("正在压缩EPUB文件: {}", epub_filename);
        
        // 创建ZIP文件
        let file = File::create(&epub_path)?;
        let mut zip = ZipWriter::new(file);
        
        // EPUB标准要求mimetype文件必须第一个添加且不压缩
        let mimetype_path = epub_dir.join("mimetype");
        if mimetype_path.exists() {
            let options: FileOptions<'_, ()> = FileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file("mimetype", options)?;
            let mimetype_content = fs::read(&mimetype_path)?;
            zip.write_all(&mimetype_content)?;
        }
        
        // 递归添加目录中的所有文件
        self.add_directory_to_zip(&mut zip, epub_dir, "")?;
        
        // 完成ZIP文件
        zip.finish()?;
        
        println!("EPUB文件已生成: {}", epub_path.display());
        
        // 删除EPUB文件夹
        println!("正在清理临时文件夹: {}", epub_dir.display());
        match fs::remove_dir_all(epub_dir) {
            Ok(()) => println!("清理成功"),
            Err(e) => println!("清理失败: {}", e),
        }
        
        Ok(epub_filename)
    }
    
    /// 递归添加目录到ZIP文件
    fn add_directory_to_zip(&self, zip: &mut ZipWriter<File>, dir: &Path, base_path: &str) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            
            // 跳过mimetype文件，因为它已经单独处理过了
            if file_name_str == "mimetype" && base_path.is_empty() {
                continue;
            }
            
            if path.is_dir() {
                // 递归处理子目录
                let new_base_path = if base_path.is_empty() {
                    file_name_str.to_string()
                } else {
                    format!("{}/{}", base_path, file_name_str)
                };
                self.add_directory_to_zip(zip, &path, &new_base_path)?;
            } else {
                // 添加文件到ZIP
                let zip_path = if base_path.is_empty() {
                    file_name_str.to_string()
                } else {
                    format!("{}/{}", base_path, file_name_str)
                };
                
                zip.start_file(&zip_path, FileOptions::<'_, ()>::default())?;
                let file_content = fs::read(&path)?;
                zip.write_all(&file_content)?;
                
                println!("已添加文件: {}", zip_path);
            }
        }
        Ok(())
    }
}