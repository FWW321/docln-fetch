use anyhow::Result;
use scraper::{Html, Selector, Element};
use crate::epub::Epub;
use crate::epub::chapter::Chapter;

pub struct NovelParser;

impl NovelParser {
    pub fn parse_novel_info(&self, html_content: &str, url: &str, novel_id: u32) -> Result<Epub> {
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

        // 解析标签
        let mut tags = Vec::new();
        let tags_selector = Selector::parse("div.series-gernes > a").unwrap();
        for tag_element in document.select(&tags_selector) {
            let tag_text = tag_element.text().collect::<String>().trim().to_string();
            if !tag_text.is_empty() {
                tags.push(tag_text);
            }
        }

        // 创建Epub结构体（其他字段将在后续处理中填充）
        let epub = Epub {
            id: novel_id,
            title,
            author,
            illustrator,
            summary,
            cover_image_path: None,
            volumes: Vec::new(),
            tags,
            url: url.to_string(),
        };

        Ok(epub)
    }

    pub fn extract_cover_url(&self, document: &Html) -> Option<String> {
        let cover_selector = Selector::parse("div.content.img-in-ratio").unwrap();
        if let Some(cover_div) = document.select(&cover_selector).next() {
            if let Some(style) = cover_div.value().attr("style") {
                // 从style属性中提取URL: background-image: url('...')
                if let Some(start) = style.find("url('") {
                    let start = start + 5; // 跳过 "url('"
                    if let Some(end) = style[start..].find("')") {
                        let image_url = &style[start..start + end];
                        return Some(image_url.to_string());
                    }
                }
            }
        }
        None
    }

    pub fn parse_volume_info(&self, document: &Html) -> Vec<(String, String)> {
        let mut volumes = Vec::new();
        let list_vol_section_selector = Selector::parse("section#list-vol").unwrap();
        let list_volume_selector = Selector::parse("ol.list-volume").unwrap();
        let volume_item_selector = Selector::parse("li").unwrap();
        let volume_title_selector = Selector::parse("span.list_vol-title").unwrap();
        
        if let Some(list_vol_section) = document.select(&list_vol_section_selector).next() {
            if let Some(list_volume) = list_vol_section.select(&list_volume_selector).next() {
                for volume_item in list_volume.select(&volume_item_selector) {
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
                        volumes.push((volume_title, volume_id));
                    }
                }
            }
        }
        
        volumes
    }

    pub fn parse_volume_chapters(&self, document: &Html, volume_id: &str) -> Vec<Chapter> {
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

    pub fn extract_volume_cover_url(&self, document: &Html, volume_id: &str) -> Option<String> {
        let volume_element_id = volume_id.trim_start_matches('#');
        let volume_header_selector = Selector::parse(&format!("header#{}", volume_element_id)).unwrap();
        let volume_cover_selector = Selector::parse("div.volume-cover div.content.img-in-ratio").unwrap();
        
        if let Some(volume_header) = document.select(&volume_header_selector).next() {
            if let Some(parent_element) = volume_header.parent_element() {
                if let Some(cover_div) = parent_element.select(&volume_cover_selector).next() {
                    if let Some(style) = cover_div.value().attr("style") {
                        // 从style属性中提取URL: background-image: url('...')
                        if let Some(start) = style.find("url('") {
                            let start = start + 5; // 跳过 "url('"
                            if let Some(end) = style[start..].find("')") {
                                let image_url = &style[start..start + end];
                                return Some(image_url.to_string());
                            }
                        }
                    }
                }
            }
        }
        None
    }
}