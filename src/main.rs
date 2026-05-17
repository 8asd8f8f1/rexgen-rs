mod app;
mod calculate;
mod cli;
mod corpus;
mod error;
mod generate;
mod model;

fn main() {
    if let Err(err) = app::run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
