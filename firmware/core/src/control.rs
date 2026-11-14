mod command;
mod line;
#[cfg(test)]
mod tests;

use crate::settings::{InvalidValue, Setting, Settings, Value};
use command::{Command, ParseError};
use core::fmt::{self, Write};
use line::{LineBuffer, LineTooLong};

const BS: u8 = 0x08;
const DEL: u8 = 0x7F;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Bootsel,
}

pub trait Generator {
    fn apply(&mut self, settings: &Settings);
}

pub struct Controller<const LINE: usize> {
    line: LineBuffer<LINE>,
    after_cr: bool,
    settings: Settings,
    version: &'static str,
}

impl<const LINE: usize> Controller<LINE> {
    pub const fn new(settings: Settings, version: &'static str) -> Self {
        Self {
            line: LineBuffer::new(),
            after_cr: false,
            settings,
            version,
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn feed(
        &mut self,
        byte: u8,
        generator: &mut impl Generator,
        out: &mut impl Write,
    ) -> Option<Action> {
        let after_cr = core::mem::replace(&mut self.after_cr, byte == b'\r');
        match byte {
            b'\n' if after_cr => None,
            b'\r' | b'\n' => {
                let result = match self.line.contents() {
                    Ok([]) => None,
                    Ok(line) => Some(execute(&mut self.settings, self.version, line, generator)),
                    Err(LineTooLong) => Some(Err(Error::LineTooLong)),
                };
                let action = match result {
                    None => None,
                    Some(Ok(reply)) => {
                        let _ = write!(out, "{reply}\r\n");
                        matches!(reply, Reply::Bootsel).then_some(Action::Bootsel)
                    }
                    Some(Err(error)) => {
                        let _ = write!(out, "ERR {error}\r\n");
                        None
                    }
                };
                self.line.clear();
                action
            }
            BS | DEL => {
                self.line.backspace();
                None
            }
            _ => {
                self.line.push(byte);
                None
            }
        }
    }
}

fn execute<'s>(
    settings: &'s mut Settings,
    version: &'static str,
    line: &[u8],
    generator: &mut impl Generator,
) -> Result<Reply<'s>, Error> {
    match command::parse(line)? {
        Command::Set(setting, value) => {
            settings.set(setting, value)?;
            generator.apply(settings);
        }
        Command::Get(Some(setting)) => return Ok(Reply::Value(settings.value(setting))),
        Command::Get(None) => return Ok(Reply::Settings(settings)),
        Command::Defaults => {
            *settings = Settings::default();
            generator.apply(settings);
        }
        Command::Ver => return Ok(Reply::Version(version)),
        Command::Bootsel => return Ok(Reply::Bootsel),
        Command::Help => return Ok(Reply::Help),
    }
    Ok(Reply::Ok)
}

enum Reply<'a> {
    Ok,
    Bootsel,
    Value(Value<'a>),
    Settings(&'a Settings),
    Version(&'static str),
    Help,
}

impl fmt::Display for Reply<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OK")?;
        match self {
            Reply::Ok | Reply::Bootsel => Ok(()),
            Reply::Value(value) => write!(f, " {value}"),
            Reply::Settings(settings) => Setting::ALL.into_iter().try_for_each(|setting| {
                write!(f, " {}={}", setting.name(), settings.value(setting))
            }),
            Reply::Version(version) => write!(f, " {version}"),
            Reply::Help => command::NAMES
                .into_iter()
                .try_for_each(|(name, _)| write!(f, " {name}")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Error {
    UnknownCommand,
    BadArgument,
    UnknownSetting,
    BadValue,
    LineTooLong,
}

impl From<ParseError> for Error {
    fn from(error: ParseError) -> Self {
        match error {
            ParseError::UnknownCommand => Error::UnknownCommand,
            ParseError::BadArgument => Error::BadArgument,
            ParseError::UnknownSetting => Error::UnknownSetting,
        }
    }
}

impl From<InvalidValue> for Error {
    fn from(_: InvalidValue) -> Self {
        Error::BadValue
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Error::UnknownCommand => "unknown command",
            Error::BadArgument => "bad argument",
            Error::UnknownSetting => "unknown setting",
            Error::BadValue => "bad value",
            Error::LineTooLong => "line too long",
        })
    }
}
