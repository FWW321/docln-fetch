use std::error::Error;
use std::io::{self, Write};
use docln_fetch::{DoclnCrawler, get_user_input};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let crawler = DoclnCrawler::new();
    
    loop {
        println!("\n=== docln-fetch ===");
        match get_user_input() {
            Ok(novel_id) => {
                println!("\n正在爬取 ID为 {} 的小说...", novel_id);
                crawler.crawl_novel(novel_id).await;
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