use anyhow::Result;
use std::fs;
use std::path::Path;
use crate::crawler::Volume;

pub struct ChapterGenerator;

impl ChapterGenerator {
    pub fn new() -> Self {
        Self
    }

    /// 生成卷封面章节0的XHTML文件
    pub fn generate_volume_cover_chapter(&self, volume: &Volume, volume_index: usize, oebps_dir: &Path) -> Result<()> {
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

    /// 为所有有卷封面的卷生成章节0 XHTML文件
    pub fn generate_all_volume_cover_chapters(&self, novel_info: &crate::crawler::NovelInfo, oebps_dir: &Path) -> Result<()> {
        for (i, volume) in novel_info.volumes.iter().enumerate() {
            if volume.cover_image_path.is_some() {
                self.generate_volume_cover_chapter(volume, i, oebps_dir)?;
            }
        }
        Ok(())
    }
}