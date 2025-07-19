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
    settings::{Gui},
    string::ArrayCString,
    timer::{self, TimerState},
    watcher::Watcher
};

asr::async_main!(stable);
asr::panic_handler!();

const pNames: &[&str] = &["SniperEliteV2.exe", "SEV2_Remastered.exe", "SniperEliteV2_D3D11_UWP_Retail_Submission.exe" , "MainThread"]; //MainThread = Wine placeholder

#[derive(Gui)]
struct Settings {
    #[default = false]
    Individual_level: bool,
    #[default = false]
    Slow_PC_mode: bool
}

#[derive(Default)]
struct Watchers {
    startByte: Watcher<u8>,
    ilStartByte: Watcher<u8>,
    loadByte: Watcher<u8>,
    splashByte: Watcher<u8>,
    level: Watcher<ArrayCString<2>>,
    bulletCam: Watcher<u8>,
    objective: Watcher<u8>,
    mc: Watcher<u8>
}

struct Memory {
    start: Address,
    ilStart: Address,
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
            Err(_) => match process.get_module_address("SEV2_Remastered.exe") {
                Ok(baseModule) => baseModule,
                Err(_) => process.get_module_address("SniperEliteV2_D3D11_UWP_Retail_Submission.exe").unwrap()
            }
        };
        let baseModuleSize = retry(|| pe::read_size_of_image(process, baseModule)).await;
        //asr::print_limited::<128>(&format_args!("{}", baseModuleSize));

        match baseModuleSize {
            18169856 => Self { //Remastered(Win32)
                start: baseModule + 0x799A77,
                ilStart: baseModule + 0x767308,
                load: baseModule + 0x774FE3,
                splash: baseModule + 0x74C670,
                level: baseModule + 0x7CFC7D,
                bullet: baseModule + 0x76DD17,
                objective: baseModule + 0x7CF568,
                mc: baseModule + 0x799A63
            },
            21979136 => Self { //Remastered(UWP)
                start: baseModule + 0xB55BE7,
                ilStart: baseModule + 0xAAFD08,
                load: baseModule + 0xB31147,
                splash: baseModule + 0xA95184,
                level: baseModule + 0xB8368D,
                bullet: baseModule + 0xAB62DF,
                objective: baseModule + 0xB82F68,
                mc: baseModule + 0xB55BD3
            },
            _ => Self { //Original
                start: baseModule + 0x689FE2,
                ilStart: baseModule + 0x649458,
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
        true => watchers.ilStartByte.pair.unwrap().changed_from_to(&0, &1)
        && !watchers.level.pair.unwrap().current.matches("nu"),
        false => watchers.startByte.pair.unwrap().changed_to(&1)
    }
}

fn isLoading(watchers: &Watchers, _settings: &Settings) -> Option<bool> {
    Some(watchers.loadByte.pair?.current == 0 || watchers.splashByte.pair?.current == 0)
}

fn split(watchers: &Watchers, settings: &Settings) -> bool {
    match settings.Individual_level {
        true => watchers.mc.pair.unwrap().changed_to(&1),
        false => {
            let level = watchers.level.pair.unwrap();

            level.changed()
            && !level.current.is_empty()
            && !level.current.matches("nu")
            && !level.current.matches("Tu")
            || level.current.matches("Br")
            && watchers.bulletCam.pair.unwrap().current == 1
            && watchers.objective.pair.unwrap().current == 3
        }
    }
}

fn mainLoop(process: &Process, memory: &Memory, watchers: &mut Watchers, settings: &Settings) {
    match settings.Individual_level {
        true => {
            watchers.ilStartByte.update_infallible(process.read(memory.ilStart).unwrap_or(0));
            watchers.mc.update_infallible(process.read(memory.mc).unwrap_or(0))
        },
        false => watchers.startByte.update_infallible(process.read(memory.start).unwrap_or(0))
    };

    watchers.loadByte.update_infallible(process.read(memory.load).unwrap_or(1));
    watchers.splashByte.update_infallible(process.read(memory.splash).unwrap_or(1));

    watchers.bulletCam.update_infallible(process.read(memory.bullet).unwrap_or(0));
    watchers.objective.update_infallible(process.read(memory.objective).unwrap_or(0));

    watchers.level.update_infallible(process.read(memory.level).unwrap_or_default());
}

async fn main() {
    let mut settings = Settings::register();

    asr::set_tick_rate(60.0);
    let mut tickToggled = false;

    loop {
        let process = retry(|| pNames.iter().find_map(|&name| Process::attach(name))).await;

        process.until_closes(async {
            let mut watchers = Watchers::default();
            let memory = Memory::init(&process).await;

            loop {
                settings.update();

                if settings.Slow_PC_mode && !tickToggled {
                    asr::set_tick_rate(30.0);
                    tickToggled = true;
                }
                else if !settings.Slow_PC_mode && tickToggled {
                    asr::set_tick_rate(60.0);
                    tickToggled = false;
                }

                mainLoop(&process, &memory, &mut watchers, &settings);

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

                next_tick().await;
            }
        }).await;

        if timer::state().eq(&TimerState::Running) {
            timer::pause_game_time();
        }
    }
}