use std::error::Error;
use rmcp::serve_server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  println!("Hello, MCP Server!");

  let io = (tokio::io::stdin(), tokio::io::stdout());
  Ok(())
}
