use anyhow::Result;
use std::fs;
use std::path::Path;

pub struct ImageDownloader {
    client: reqwest::Client,
}

impl ImageDownloader {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// 通用的图片下载函数
    pub async fn download_image(
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

    /// 通用的封面图片下载函数
    pub async fn download_cover_image_common(
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

    pub async fn download_novel_cover(
        &self,
        image_url: &str,
        _novel_id: u32,
        _title: &str,
        epub_dir: &Path,
    ) -> Result<Option<String>> {
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

    pub async fn download_volume_cover_image(
        &self,
        image_url: &str,
        _volume_index: usize,
        volume_title: &str,
        epub_dir: &Path,
    ) -> Result<Option<String>> {
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
}