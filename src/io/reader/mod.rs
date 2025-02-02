pub mod command;
pub mod linemux;
pub mod stdin;

use async_trait::async_trait;
use tokio::io;

#[async_trait]
pub trait AsyncLineReader {
    async fn next_line(&mut self) -> io::Result<Option<String>>;
}
