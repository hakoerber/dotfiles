use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("no action given")]
    MissingAction,
    #[error("unknown action: {action}")]
    UnknownAction { action: String },
    #[error("unexpected input: {rest:?}")]
    UnexpectedInput { rest: Vec<String> },
    #[error("missing argument")]
    MissingArgument,
    #[error("error parsing argument: {message}")]
    ArgumentParse { message: String },
}

pub trait CliCommand {
    type ExecErr: From<ParseError>;

    fn parse_str(input: &str, rest: impl Iterator<Item = String>) -> Result<Self, ParseError>
    where
        Self: Sized;
}
