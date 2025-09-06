use std::error::Error;
use std::io;

pub fn get_user_input() -> Result<u32, Box<dyn Error>> {
    println!("请输入小说ID: ");
    let mut novel_id = String::new();
    io::stdin().read_line(&mut novel_id)?;
    let novel_id: u32 = novel_id.trim().parse()
        .map_err(|_| "请输入有效的小说ID (数字)")?;
    
    Ok(novel_id)
}

