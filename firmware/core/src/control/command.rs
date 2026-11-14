use crate::{keyword::parse_keyword, settings::Setting};

#[derive(Clone, Copy)]
pub(super) enum Name {
    Set,
    Get,
    Defaults,
    Ver,
    Bootsel,
    Help,
}

pub(super) const NAMES: [(&str, Name); 6] = [
    ("SET", Name::Set),
    ("GET", Name::Get),
    ("DEFAULTS", Name::Defaults),
    ("VER", Name::Ver),
    ("BOOTSEL", Name::Bootsel),
    ("HELP", Name::Help),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Command<'a> {
    Set(Setting, &'a [u8]),
    Get(Option<Setting>),
    Defaults,
    Ver,
    Bootsel,
    Help,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ParseError {
    UnknownCommand,
    BadArgument,
    UnknownSetting,
}

pub(super) fn parse(line: &[u8]) -> Result<Command<'_>, ParseError> {
    let mut words = line.split(|&b| b == b' ');
    let name = words.next().unwrap_or_default();
    let name = parse_keyword(name, &NAMES).ok_or(ParseError::UnknownCommand)?;
    let mut args = words.filter(|arg| !arg.is_empty());
    let command = match name {
        Name::Set => {
            let setting = setting(args.next().ok_or(ParseError::BadArgument)?)?;
            Command::Set(setting, args.next().ok_or(ParseError::BadArgument)?)
        }
        Name::Get => Command::Get(args.next().map(setting).transpose()?),
        Name::Defaults => Command::Defaults,
        Name::Ver => Command::Ver,
        Name::Bootsel => Command::Bootsel,
        Name::Help => Command::Help,
    };
    match args.next() {
        Some(_) => Err(ParseError::BadArgument),
        None => Ok(command),
    }
}

fn setting(name: &[u8]) -> Result<Setting, ParseError> {
    Setting::from_name(name).ok_or(ParseError::UnknownSetting)
}

#[cfg(test)]
mod tests {
    use super::{Command, ParseError, parse};
    use crate::settings::Setting;
    use rstest::rstest;

    #[rstest]
    #[case("SET FREQ 1000", Command::Set(Setting::Freq, b"1000"))]
    #[case("set  wave   square ", Command::Set(Setting::Wave, b"square"))]
    #[case("Set Amp 1.5", Command::Set(Setting::Amp, b"1.5"))]
    #[case("GET", Command::Get(None))]
    #[case("GET ", Command::Get(None))]
    #[case("get freq", Command::Get(Some(Setting::Freq)))]
    #[case("DEFAULTS", Command::Defaults)]
    #[case("VER", Command::Ver)]
    #[case("BOOTSEL", Command::Bootsel)]
    #[case("help", Command::Help)]
    fn parses_commands(#[case] line: &str, #[case] command: Command) {
        assert_eq!(parse(line.as_bytes()), Ok(command));
    }

    #[rstest]
    #[case("FREQ 1000", ParseError::UnknownCommand)]
    #[case(" SET FREQ 1000", ParseError::UnknownCommand)]
    #[case("SETFREQ 1000", ParseError::UnknownCommand)]
    #[case("SET", ParseError::BadArgument)]
    #[case("SET FREQ", ParseError::BadArgument)]
    #[case("SET FREQ 10 00", ParseError::BadArgument)]
    #[case("SET OFFSET 1", ParseError::UnknownSetting)]
    #[case("GET OFFSET", ParseError::UnknownSetting)]
    #[case("GET FREQ AMP", ParseError::BadArgument)]
    #[case("VER 1", ParseError::BadArgument)]
    fn rejects_bad_lines(#[case] line: &str, #[case] error: ParseError) {
        assert_eq!(parse(line.as_bytes()), Err(error));
    }
}
