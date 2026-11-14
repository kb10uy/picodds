use super::{Action, Controller, Generator};
use crate::{
    settings::{Setting, Settings},
    waveform::Waveform,
};
use rstest::{fixture, rstest};

const LINE: usize = 32;

#[derive(Default)]
struct MockGenerator {
    applied: Vec<Settings>,
}

impl Generator for MockGenerator {
    fn apply(&mut self, settings: &Settings) {
        self.applied.push(*settings);
    }
}

struct Unit {
    controller: Controller<LINE>,
    generator: MockGenerator,
}

impl Unit {
    fn send(&mut self, input: &str) -> (String, Vec<Action>) {
        let mut out = String::new();
        let actions = input
            .bytes()
            .filter_map(|byte| self.controller.feed(byte, &mut self.generator, &mut out))
            .collect();
        (out, actions)
    }

    fn reply(&mut self, input: &str) -> String {
        self.send(input).0
    }
}

#[fixture]
fn unit() -> Unit {
    Unit {
        controller: Controller::new(Settings::default(), "0.1.0"),
        generator: MockGenerator::default(),
    }
}

fn settings_with(changes: &[(Setting, &str)]) -> Settings {
    let mut settings = Settings::default();
    for &(setting, value) in changes {
        settings.set(setting, value.as_bytes()).unwrap();
    }
    settings
}

#[rstest]
fn sets_and_applies(mut unit: Unit) {
    assert_eq!(unit.reply("SET FREQ 440\r"), "OK\r\n");
    assert_eq!(unit.reply("set wave tri\r"), "ERR bad value\r\n");
    assert_eq!(unit.reply("set wave triangle\r"), "OK\r\n");
    assert_eq!(unit.controller.settings().waveform, Waveform::Triangle);
    assert_eq!(
        unit.generator.applied,
        [
            settings_with(&[(Setting::Freq, "440")]),
            settings_with(&[(Setting::Freq, "440"), (Setting::Wave, "TRIANGLE")]),
        ]
    );
}

#[rstest]
fn gets_settings(mut unit: Unit) {
    assert_eq!(unit.reply("GET\r"), "OK WAVE=SINE FREQ=1000 AMP=1\r\n");
    assert_eq!(unit.reply("SET AMP 0.25\rGET AMP\r"), "OK\r\nOK 0.25\r\n");
    assert_eq!(unit.reply("get freq\n"), "OK 1000\r\n");
}

#[rstest]
fn restores_defaults(mut unit: Unit) {
    unit.reply("SET FREQ 12.5\rSET WAVE SAW\r");
    assert_eq!(unit.reply("DEFAULTS\r"), "OK\r\n");
    assert_eq!(unit.controller.settings(), &Settings::default());
    assert_eq!(unit.generator.applied.last(), Some(&Settings::default()));
}

#[rstest]
#[case("FREQ 1000\r", "ERR unknown command\r\n")]
#[case("SET FREQ\r", "ERR bad argument\r\n")]
#[case("SET PHASE 90\r", "ERR unknown setting\r\n")]
#[case("SET FREQ 0\r", "ERR bad value\r\n")]
#[case("SET AMP 2.5\r", "ERR bad value\r\n")]
fn reports_errors_without_applying(mut unit: Unit, #[case] input: &str, #[case] reply: &str) {
    assert_eq!(unit.reply(input), reply);
    assert!(unit.generator.applied.is_empty());
    assert_eq!(unit.controller.settings(), &Settings::default());
}

#[rstest]
#[case("VER\r\n", "OK 0.1.0\r\n")]
#[case("HELP\n", "OK SET GET DEFAULTS VER BOOTSEL HELP\r\n")]
#[case("\r\n\r\n", "")]
#[case("\n\r", "")]
#[case("VER\r\r", "OK 0.1.0\r\n")]
#[case("VER\n\n", "OK 0.1.0\r\n")]
#[case("VEX\x08R\r", "OK 0.1.0\r\n")]
#[case("VERX\x7F\r", "OK 0.1.0\r\n")]
#[case("\x08\x08VER\r", "OK 0.1.0\r\n")]
fn frames_lines(mut unit: Unit, #[case] input: &str, #[case] reply: &str) {
    assert_eq!(unit.reply(input), reply);
}

#[rstest]
fn rejects_long_lines_whole(mut unit: Unit) {
    let long = format!("SET FREQ 1{}\r", "0".repeat(LINE));
    assert_eq!(unit.reply(&long), "ERR line too long\r\n");
    assert!(unit.generator.applied.is_empty());
    assert_eq!(unit.reply("VER\r"), "OK 0.1.0\r\n");
}

#[rstest]
fn accepts_a_line_that_fills_the_buffer(mut unit: Unit) {
    let padded = format!("SET FREQ{:width$}5\r", "", width = LINE - 9);
    assert_eq!(unit.reply(&padded), "OK\r\n");
}

#[rstest]
fn asks_for_bootsel(mut unit: Unit) {
    assert_eq!(
        unit.send("BOOTSEL\r"),
        ("OK\r\n".into(), vec![Action::Bootsel])
    );
}
