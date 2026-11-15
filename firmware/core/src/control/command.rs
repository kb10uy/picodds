use crate::{
    custom::{Format, parse_slot},
    keyword::parse_keyword,
    settings::Setting,
};

#[derive(Clone, Copy)]
pub(super) enum Name {
    Set,
    Get,
    Defaults,
    WSet,
    WGet,
    WClear,
    WSave,
    WWipe,
    Ver,
    Bootsel,
    Help,
}

pub(super) const NAMES: [(&str, Name); 11] = [
    ("SET", Name::Set),
    ("GET", Name::Get),
    ("DEFAULTS", Name::Defaults),
    ("WSET", Name::WSet),
    ("WGET", Name::WGet),
    ("WCLEAR", Name::WClear),
    ("WSAVE", Name::WSave),
    ("WWIPE", Name::WWipe),
    ("VER", Name::Ver),
    ("BOOTSEL", Name::Bootsel),
    ("HELP", Name::Help),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Command<'a> {
    Set(Setting, &'a [u8]),
    Get(Option<Setting>),
    Defaults,
    WSet(u8, Format, &'a [u8]),
    WGet(u8),
    WClear(u8),
    WSave,
    WWipe,
    Ver,
    Bootsel,
    Help,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ParseError {
    UnknownCommand,
    BadArgument,
    UnknownSetting,
    BadValue,
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
        Name::WSet => {
            let slot = slot(args.next())?;
            let format = args.next().ok_or(ParseError::BadArgument)?;
            let format = Format::parse(format).ok_or(ParseError::BadValue)?;
            Command::WSet(slot, format, args.next().ok_or(ParseError::BadArgument)?)
        }
        Name::WGet => Command::WGet(slot(args.next())?),
        Name::WClear => Command::WClear(slot(args.next())?),
        Name::WSave => Command::WSave,
        Name::WWipe => Command::WWipe,
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

fn slot(arg: Option<&[u8]>) -> Result<u8, ParseError> {
    parse_slot(arg.ok_or(ParseError::BadArgument)?).ok_or(ParseError::BadValue)
}

#[cfg(test)]
mod tests {
    use super::{Command, ParseError, parse};
    use crate::{custom::Format, settings::Setting};
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
    #[case("WSET 3 I8 7F80", Command::WSet(3, Format::I8, b"7F80"))]
    #[case("wset 15 i16 abcd", Command::WSet(15, Format::I16, b"abcd"))]
    #[case("WGET 0", Command::WGet(0))]
    #[case("WCLEAR 9", Command::WClear(9))]
    #[case("WSAVE", Command::WSave)]
    #[case("wwipe", Command::WWipe)]
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
    #[case("WSET", ParseError::BadArgument)]
    #[case("WSET 1", ParseError::BadArgument)]
    #[case("WSET 1 I8", ParseError::BadArgument)]
    #[case("WSET 1 I8 00 00", ParseError::BadArgument)]
    #[case("WSET 16 I8 00", ParseError::BadValue)]
    #[case("WSET 1 U8 00", ParseError::BadValue)]
    #[case("WGET", ParseError::BadArgument)]
    #[case("WGET W1", ParseError::BadValue)]
    #[case("WCLEAR 1 2", ParseError::BadArgument)]
    #[case("WSAVE 1", ParseError::BadArgument)]
    #[case("WWIPE 1", ParseError::BadArgument)]
    fn rejects_bad_lines(#[case] line: &str, #[case] error: ParseError) {
        assert_eq!(parse(line.as_bytes()), Err(error));
    }
}
