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
    watcher::Watcher,
    signature::Signature
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
    speedFloat: Watcher<f32>,
    mc: Watcher<u8>
}

struct Memory {
    start: Address,
    ilStart: Address,
    load: Address,
    splash: Address,
    level: Address,
    speed: Address,
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
                speed: baseModule + 0x798074,
                mc: baseModule + 0x799A63
            },
            21979136 => Self { //Remastered(UWP)
                start: baseModule + 0xB55BE7,
                ilStart: baseModule + 0xAAFD08,
                load: baseModule + 0xB31147,
                splash: baseModule + 0xA95184,
                level: baseModule + 0xB8368D,
                speed: baseModule + 0xB5420C,
                mc: baseModule + 0xB55BD3
            },
            _ => { //Original
                const startAndMcSIG: Signature<12> = Signature::new("8A ?? ?? ?? ?? ?? 24 ?? 5F 5E 5D C3");
                const ilStartSIG: Signature<13> = Signature::new("A2 ?? ?? ?? ?? E8 ?? ?? ?? ?? 84 ?? 79");
                const loadSIG: Signature<38> = Signature::new("3B ?? ?? ?? ?? ?? 73 ?? 8B ?? ?? ?? ?? ?? 8B ?? ?? ?? ?? ?? 8B ?? ?? ?? ?? ?? 8B ?? ?? ?? ?? ?? 89 ?? ?? 8B ?? 51");
                const splashSIG: Signature<14> = Signature::new("A1 ?? ?? ?? ?? 39 ?? ?? ?? ?? ?? 74 ?? 50");
                const levelSIG: Signature<26> = Signature::new("68 ?? ?? ?? ?? 6A ?? 8B ?? 6A ?? 8D ?? ?? 68 ?? ?? ?? ?? 50 E8 ?? ?? ?? ?? 8B");
                const speedSIG: Signature<32> = Signature::new("F3 ?? ?? ?? ?? ?? ?? ?? F3 ?? ?? ?? ?? D9 ?? ?? 51 8D ?? ?? ?? ?? ?? D9 ?? ?? E8 ?? ?? ?? ?? D8");

                let startScan = startAndMcSIG.scan_process_range(process, (baseModule, baseModuleSize.into())).unwrap() + 2;
                let ilStartScan = ilStartSIG.scan_process_range(process, (baseModule, baseModuleSize.into())).unwrap() + 1;
                let loadScan = loadSIG.scan_process_range(process, (baseModule, baseModuleSize.into())).unwrap() + 2;
                let splashScan = splashSIG.scan_process_range(process, (baseModule, baseModuleSize.into())).unwrap() + 1;
                let levelScan = levelSIG.scan_process_range(process, (baseModule, baseModuleSize.into())).unwrap() + 1;
                let speedScan = speedSIG.scan_process_range(process, (baseModule, baseModuleSize.into())).unwrap() + 4;

                Self {
                    start: (process.read::<u32>(startScan).unwrap() + 0x22).into(),
                    ilStart: process.read::<u32>(ilStartScan).unwrap().into(),
                    load: process.read::<u32>(loadScan).unwrap().into(),
                    splash: process.read::<u32>(splashScan).unwrap().into(),
                    level: (process.read::<u32>(levelScan).unwrap() + 0x5).into(),
                    speed: process.read::<u32>(speedScan).unwrap().into(),
                    mc: (process.read::<u32>(startScan).unwrap() + 0x12).into()
                }
            }
        }
    }
}

fn start(watchers: &Watchers, settings: &Settings) -> bool {
    match settings.Individual_level {
        true => watchers.ilStartByte.pair.unwrap().changed_from_to(&0, &1),
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
            || level.current.matches("Br")
            && watchers.speedFloat.pair.unwrap().current == 0.25
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

    watchers.speedFloat.update_infallible(process.read(memory.speed).unwrap_or(1.0));

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