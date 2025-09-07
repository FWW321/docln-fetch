pub mod crawler;
pub mod epub;
pub mod utils;

pub use crawler::{NovelInfo, Volume, Chapter, DoclnCrawler};
pub use epub::EpubBuilder;
pub use utils::get_user_input;