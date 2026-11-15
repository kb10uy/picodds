use super::{Action, Controller, Generator, LINE_BYTES};
use crate::{
    custom::{Bank, Format, MAX_SAMPLES, MemoryFlash},
    settings::{Setting, Settings},
    waveform::Waveform,
};
use rstest::{fixture, rstest};

const LINE: usize = LINE_BYTES;

#[derive(Default)]
struct MockGenerator {
    applied: Vec<Settings>,
    mutes: Vec<bool>,
}

impl Generator for MockGenerator {
    fn apply(&mut self, settings: &Settings) {
        self.applied.push(*settings);
    }

    fn mute(&mut self) {
        self.mutes.push(true);
    }

    fn unmute(&mut self) {
        self.mutes.push(false);
    }
}

struct Unit {
    controller: Controller<'static, LINE>,
    generator: MockGenerator,
    flash: MemoryFlash,
}

impl Unit {
    fn send(&mut self, input: &str) -> (String, Vec<Action>) {
        let mut out = String::new();
        let actions = input
            .bytes()
            .filter_map(|byte| {
                self.controller
                    .feed(byte, &mut self.generator, &mut self.flash, &mut out)
            })
            .collect();
        (out, actions)
    }

    fn reply(&mut self, input: &str) -> String {
        self.send(input).0
    }
}

#[fixture]
fn unit() -> Unit {
    let bank = Box::leak(Box::new(Bank::new()));
    Unit {
        controller: Controller::new(Settings::default(), bank, "0.1.0"),
        generator: MockGenerator::default(),
        flash: MemoryFlash::new(),
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
#[case("SET WAVE W16\r", "ERR bad value\r\n")]
fn reports_errors_without_applying(mut unit: Unit, #[case] input: &str, #[case] reply: &str) {
    assert_eq!(unit.reply(input), reply);
    assert!(unit.generator.applied.is_empty());
    assert_eq!(unit.controller.settings(), &Settings::default());
}

#[rstest]
#[case("VER\r\n", "OK 0.1.0\r\n")]
#[case(
    "HELP\n",
    "OK SET GET DEFAULTS WSET WGET WCLEAR WSAVE WWIPE VER BOOTSEL HELP\r\n"
)]
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

#[rstest]
fn selects_custom_slots(mut unit: Unit) {
    assert_eq!(unit.reply("SET WAVE w12\rGET WAVE\r"), "OK\r\nOK W12\r\n");
    assert_eq!(unit.controller.settings().waveform, Waveform::Custom(12));
}

#[rstest]
fn sets_gets_and_clears_slots(mut unit: Unit) {
    assert_eq!(unit.reply("WGET 4\r"), "OK EMPTY\r\n");
    assert_eq!(unit.reply("WSET 4 I8 80007f\r"), "OK\r\n");
    assert_eq!(unit.reply("WGET 4\r"), "OK I8 80007F\r\n");
    assert_eq!(
        unit.reply("WSET 4 I16 1234\rWGET 4\r"),
        "OK\r\nOK I16 1234\r\n"
    );
    assert_eq!(unit.reply("WCLEAR 4\rWGET 4\r"), "OK\r\nOK EMPTY\r\n");
    assert!(unit.generator.applied.is_empty());
}

#[rstest]
#[case("WSET 4 I8 123\r", "ERR bad value\r\n")]
#[case("WSET 4 I16 zz00\r", "ERR bad value\r\n")]
#[case("WSET 16 I8 00\r", "ERR bad value\r\n")]
#[case("WSET 4 I8\r", "ERR bad argument\r\n")]
fn rejects_bad_slot_data(mut unit: Unit, #[case] input: &str, #[case] reply: &str) {
    unit.reply("WSET 4 I8 01\r");
    assert_eq!(unit.reply(input), reply);
    assert_eq!(unit.reply("WGET 4\r"), "OK I8 01\r\n");
}

#[rstest]
fn takes_a_full_waveform_in_one_line(mut unit: Unit) {
    let hex = "7FFF".repeat(MAX_SAMPLES);
    assert_eq!(unit.reply(&format!("WSET 15 I16 {hex}\r")), "OK\r\n");
    assert_eq!(unit.reply("WGET 15\r"), format!("OK I16 {hex}\r\n"));
    let longer = format!("WSET 15 I16 {hex}0000\r");
    assert_eq!(unit.reply(&longer), "ERR bad value\r\n");
}

#[rstest]
fn saves_with_the_output_muted(mut unit: Unit) {
    assert_eq!(unit.reply("WSAVE\r"), "OK\r\n");
    assert!(unit.generator.mutes.is_empty());
    assert!(unit.flash.erased.is_empty());

    unit.reply("WSET 2 I8 7F\r");
    assert_eq!(unit.reply("WSAVE\r"), "OK\r\n");
    assert_eq!(unit.generator.mutes, [true, false]);
    assert_eq!(unit.flash.written, [2]);

    assert_eq!(unit.reply("WSAVE\r"), "OK\r\n");
    assert_eq!(unit.generator.mutes, [true, false]);

    let reloaded = Bank::new();
    reloaded.load(&mut unit.flash).unwrap();
    assert_eq!(reloaded.slot(2).to_string(), "I8 7F");
}

#[rstest]
fn wipes_memory_and_flash(mut unit: Unit) {
    unit.reply("WSET 2 I8 7F\rWSET 5 I16 0001\rWSAVE\rWSET 9 I8 00\r");
    assert_eq!(unit.reply("WWIPE\r"), "OK\r\n");
    assert_eq!(unit.reply("WGET 9\r"), "OK EMPTY\r\n");
    assert_eq!(unit.flash.erased, [2, 5, 2, 5]);
    assert_eq!(unit.generator.mutes, [true, false, true, false]);

    let reloaded = Bank::new();
    reloaded.slot(2).set(Format::I8, b"00").unwrap();
    reloaded.load(&mut unit.flash).unwrap();
    assert_eq!(reloaded.slot(2).to_string(), "EMPTY");
}

#[rstest]
fn keeps_slots_on_defaults(mut unit: Unit) {
    unit.reply("WSET 1 I8 7F\rSET WAVE W1\r");
    assert_eq!(unit.reply("DEFAULTS\rWGET 1\r"), "OK\r\nOK I8 7F\r\n");
}
