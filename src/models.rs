use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Chapter {
    pub title: String,
    pub url: String,
    pub has_illustrations: bool, // 是否包含插图
    pub xhtml_path: Option<String>, // XHTML文件路径（用于EPUB）
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Volume {
    pub title: String,
    pub volume_id: String,
    pub cover_image_path: Option<String>,
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NovelInfo {
    pub title: String,
    pub author: String,
    pub illustrator: Option<String>, // 插画师
    pub summary: String, // 简介内容
    pub cover_image_path: Option<String>, // 封面图片本地路径
    pub volumes: Vec<Volume>, // 卷信息
    pub tags: Vec<String>,
    pub url: String,
}