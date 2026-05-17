use crate::cli::CliApp;
use crate::corpus::Corpus;
use crate::error::Error;
use crate::error::Result;

pub(crate) struct App;

impl App {
    pub(crate) fn run() -> Result<()> {
        let matches = CliApp().get_matches();
        let pattern = matches
            .get_one::<String>("pattern")
            .ok_or_else(|| Error::Parse("No valid pattern found!"))?;

        let corpus = Corpus::new(pattern)?;

        corpus.generate()?;

        Ok(())
    }
}
