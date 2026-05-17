use crate::app::App;
use crate::error::Result;

mod app;
mod calculate;
mod cli;
mod corpus;
mod error;
mod generate;
mod model;

fn main() -> Result<()> {
    App::run()?;
    Ok(())
}
