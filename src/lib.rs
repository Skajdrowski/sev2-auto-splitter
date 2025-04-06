#![no_std]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![warn(
    clippy::complexity,
    clippy::correctness,
    clippy::perf,
    clippy::style,
    clippy::undocumented_unsafe_blocks,
    rust_2018_idioms
)]

use asr::{
    Address, Process,
    file_format::pe,
    future::{next_tick, retry},
    settings::{Gui, Map},
    string::ArrayCString,
    timer::{self, TimerState},
    watcher::Watcher
};

asr::async_main!(stable);
asr::panic_handler!();

const pNames: &[&str] = &["SniperEliteV2.exe", "SEV2_Remastered.exe", "MainThread"]; //MainThread = Wine placeholder

#[derive(Gui)]
struct Settings {
    #[default = true]
    Full_game_run: bool,
    #[default = false]
    Individual_level: bool,
    #[default = false]
    Slow_PC_mode: bool
}

#[derive(Default)]
struct Watchers {
    startByte: Watcher<u8>,
    loadByte: Watcher<u8>,
    splashByte: Watcher<u8>,
    level: Watcher<ArrayCString<2>>,
    bulletCam: Watcher<u8>,
    objective: Watcher<u8>,
    mc: Watcher<u8>
}

struct Memory {
    start: Address,
    load: Address,
    splash: Address,
    level: Address,
    bullet: Address,
    objective: Address,
    mc: Address
}

impl Memory {
    async fn init(process: &Process) -> Self {
        let baseModule = match process.get_module_address("SniperEliteV2.exe") {
            Ok(baseModule) => baseModule,
            Err(_) => process.get_module_address("SEV2_Remastered.exe").unwrap()
        };
        let baseModuleSize = retry(|| pe::read_size_of_image(process, baseModule)).await;
        //asr::print_limited::<128>(&format_args!("{}", baseModuleSize));

        match baseModuleSize {
            18169856 => Self {
                start: baseModule + 0x799A77,
                load: baseModule + 0x774FE3,
                splash: baseModule + 0x74C670,
                level: baseModule + 0x7CFC7D,
                bullet: baseModule + 0x76DD17,
                objective: baseModule + 0x7CF568,
                mc: baseModule + 0x799A63
            },
            _ => Self {
                start: baseModule + 0x689FE2,
                load: baseModule + 0x67FC38,
                splash: baseModule + 0x653B40,
                level: baseModule + 0x685F31,
                bullet: baseModule + 0x65B917,
                objective: baseModule + 0x656F3C,
                mc: baseModule + 0x689FD2
            }
        }
    }
}

fn start(watchers: &Watchers, settings: &Settings) -> bool {
    match settings.Individual_level {
        true => watchers.splashByte.pair.is_some_and(|val|
            val.changed_from_to(&0, &1)
            && watchers.level.pair.is_some_and(|val| !val.current.matches("nu"))
        ),
        false => watchers.startByte.pair.is_some_and(|val| val.changed_to(&1))
    }
}

fn isLoading(watchers: &Watchers, _settings: &Settings) -> Option<bool> {
    Some(watchers.loadByte.pair?.current == 0 || watchers.splashByte.pair?.current == 0)
}

fn split(watchers: &Watchers, settings: &Settings) -> bool {
    match settings.Individual_level {
        true => watchers.mc.pair.is_some_and(|val| val.changed_to(&1)),
        false => watchers.level.pair.is_some_and(|val|
            val.changed()
            && !val.current.is_empty()
            && !val.current.matches("nu")
            && !val.current.matches("Tu")
        )
        || (watchers.level.pair.is_some_and(|val| val.current.matches("Br"))
        && watchers.bulletCam.pair.is_some_and(|val| val.current == 1)
        && watchers.objective.pair.is_some_and(|val| val.current == 3))
    }
}

fn mainLoop(process: &Process, memory: &Memory, watchers: &mut Watchers) {
    watchers.startByte.update_infallible(process.read(memory.start).unwrap_or_default());

    watchers.loadByte.update_infallible(process.read(memory.load).unwrap_or(1));
    watchers.splashByte.update_infallible(process.read(memory.splash).unwrap_or(1));

    watchers.bulletCam.update_infallible(process.read(memory.bullet).unwrap_or_default());
    watchers.objective.update_infallible(process.read(memory.objective).unwrap_or_default());
    watchers.mc.update_infallible(process.read(memory.mc).unwrap_or_default());

    watchers.level.update_infallible(process.read(memory.level).unwrap_or_default());
}

async fn main() {
    let mut settings = Settings::register();
    let mut map = Map::load();

    asr::set_tick_rate(60.0);
    let mut tickToggled = false;

    loop {
        let process = retry(|| pNames.iter().find_map(|&name| Process::attach(name))).await;

        process.until_closes(async {
            let mut watchers = Watchers::default();
            let memory = Memory::init(&process).await;

            loop {
                settings.update();

                if settings.Full_game_run && settings.Individual_level {
                    map.store();
                }

                if settings.Slow_PC_mode && !tickToggled {
                    asr::set_tick_rate(30.0);
                    map = Map::load();
                    tickToggled = true;
                }
                else if !settings.Slow_PC_mode && tickToggled {
                    asr::set_tick_rate(60.0);
                    map = Map::load();
                    tickToggled = false;
                }

                if [TimerState::Running, TimerState::Paused].contains(&timer::state()) {
                    match isLoading(&watchers, &settings) {
                        Some(true) => timer::pause_game_time(),
                        Some(false) => timer::resume_game_time(),
                        _ => ()
                    }

                    if split(&watchers, &settings) {
                        timer::split();
                    }
                }

                if timer::state().eq(&TimerState::NotRunning) && start(&watchers, &settings) {
                    timer::start();
                }

                mainLoop(&process, &memory, &mut watchers);
                next_tick().await;
            }
        }).await;
    }
}