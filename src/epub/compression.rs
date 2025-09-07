use anyhow::Result;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use zip::write::FileOptions;
use zip::ZipWriter;

pub struct EpubCompressor;

impl EpubCompressor {
    pub fn new() -> Self {
        Self
    }

    /// 压缩EPUB文件夹为EPUB文件
    pub fn compress_epub(&self, epub_dir: &Path) -> Result<String> {
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