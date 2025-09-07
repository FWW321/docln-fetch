use anyhow::Result;
use std::fs;
use std::path::Path;
use scraper::{Html, Selector};
use super::Chapter;

pub struct ChapterProcessor {
    client: reqwest::Client,
    base_url: String,
}

impl ChapterProcessor {
    pub fn new(client: reqwest::Client, base_url: String) -> Self {
        Self { client, base_url }
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
        let extension = std::path::Path::new(image_url)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("jpg");
        
        // 插图命名为顺序编号
        let filename = format!("{:03}.{}", illustration_number, extension);
        let filepath = illustrations_dir.join(&filename);
        
        // 使用通用函数下载图片
        println!("正在下载插图 {}: {}", illustration_number, image_url);
        let response = self.client.get(image_url).send().await?;
        let image_bytes = response.bytes().await?;
        std::fs::write(&filepath, &image_bytes)?;
        println!("插图 {} 已保存到: {}", illustration_number, filepath.display());
        
        // 返回正确的相对路径（从text/volume_XXX/chapter_XXX.xhtml到images/volume_XXX/chapter_XXX/）
        Ok(format!("../../images/volume_{:03}/chapter_{:03}/{}", volume_index + 1, chapter_index + 1, filename))
    }
}