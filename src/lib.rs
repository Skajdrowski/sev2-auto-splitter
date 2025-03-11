#![no_std]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(static_mut_refs)]

use asr::{future::sleep, settings::Gui, Process};
use core::{str, time::Duration};
use dlmalloc::GlobalDlmalloc;

#[global_allocator]
static ALLOCATOR: GlobalDlmalloc = GlobalDlmalloc;

asr::async_main!(stable);
asr::panic_handler!();

#[derive(Gui)]
struct Settings {
    #[default = true]
    Full_game_run: bool,
    #[default = false]
    Individual_level: bool
}

struct Addr {
    startAddress: u32,
    loadAddress: u32,
    splashAddress: u32,
    levelAddress: u32,
    bulletCamAddress: u32,
    objectiveAddress: u32,
    mcAddress: u32
}

impl Addr {
    fn original() -> Self {
        Self {
            startAddress: 0x689FE2,
            loadAddress: 0x67FC38,
            splashAddress: 0x653B40,
            levelAddress: 0x685F31,
            bulletCamAddress: 0x65B917,
            objectiveAddress: 0x656F3C,
            mcAddress: 0x689FD2
        }
    }
    
    fn remastered() -> Self {
        Self {
            startAddress: 0x799A77,
            loadAddress: 0x774FE3,
            splashAddress: 0x74C670,
            levelAddress: 0x7CFC7D,
            bulletCamAddress: 0x76DD17,
            objectiveAddress: 0x7CF568,
            mcAddress: 0x799A63
        }
    }
}

async fn Attacher() -> Process {
    loop {
        if let Some(process) = ["SniperEliteV2.exe", "SEV2_Remastered.exe"].into_iter().find_map(Process::attach)
        {
            return process;
        }
        else {
            asr::print_message("Waiting for game to attach...");
            sleep(Duration::from_secs(1)).await;
        }
    }
}

async fn main() {
    let mut settings = Settings::register();
    
    static mut splashByte: u8 = 0;
    let mut oldSplash: u8 = 0;
    
    static mut levelStr: &str = "";
    static mut levelArray: [u8; 38] = [0; 38];
    static mut oldLevel: [u8; 38] = [0; 38];
    
    
    let mut baseAddress = asr::Address::new(0);
    let mut addrStruct = Addr::original();
    
    let process = Attacher().await;

    process.until_closes(async {
        if let Some((base, moduleSize)) = ["SniperEliteV2.exe", "SEV2_Remastered.exe"].into_iter().find_map (|exe|
        Some((process.get_module_address(exe).ok()?,
        process.get_module_size(exe).ok()?)))
        {
            baseAddress = base;
            if moduleSize == 18169856 {
                addrStruct = Addr::remastered();
            }
        }
        unsafe {
            let start = || {
                let startByte: u8 = process.read::<u8>(baseAddress + addrStruct.startAddress).unwrap_or(0);
                if startByte == 1 {
                    asr::timer::start();
                }
            };
            
            let isLoading = || {
                let loadByte: u8 = process.read::<u8>(baseAddress + addrStruct.loadAddress).unwrap_or(1); 
                splashByte = process.read::<u8>(baseAddress + addrStruct.splashAddress).unwrap_or(1);
                if loadByte == 0 || splashByte == 0 {
                    asr::timer::pause_game_time();
                }
                else {
                    asr::timer::resume_game_time();
                }
            };
                
            let levelSplit = || {
                if levelArray != oldLevel {
                    if levelStr != "" && levelStr != "nu\\Options.gui" && levelStr != "nu\\GUIMenuCommon.asr" && levelStr != "Tutorial\\M01_Tutorial.pc" {
                        asr::timer::split();
                    }
                }
            };
                
            let lastSplit = || {
                let bulletCamByte: u8 = process.read::<u8>(baseAddress + addrStruct.bulletCamAddress).unwrap_or(0); 
                let objectiveByte: u8 = process.read::<u8>(baseAddress + addrStruct.objectiveAddress).unwrap_or(0);
            
                if levelStr == "BrandenburgGate\\M11_BrandenburgGate.pc" && bulletCamByte == 1 && objectiveByte == 3 {
                    asr::timer::split();
                }
            };
                
            let mut individualLvl = || {
                let mcByte: u8 = process.read::<u8>(baseAddress + addrStruct.mcAddress).unwrap_or(0);
            
                if mcByte == 1 {
                    asr::timer::split();
                }
                if splashByte != oldSplash {
                    oldSplash = splashByte;
                }
                if (splashByte == 1 && oldSplash == 0) && levelStr != "nu\\Options.gui" {
                    asr::timer::start();
                }
            };
        
            loop {
                settings.update();
            
                match process.read_into_slice(baseAddress + addrStruct.levelAddress, &mut levelArray) {
                    Ok(_) => levelArray[0],
                    Err(_) => return
                };
                levelStr = str::from_utf8(&levelArray).unwrap_or("").split('\0').next().unwrap_or("");
                
                if settings.Full_game_run {
                    //let start_time = asr::time_util::Instant::now();
                    start();
                    //let end_time = start_time.elapsed();
                    levelSplit();
                    lastSplit();
                    //asr::print_message(&alloc::format!("Tick time: {:?}", end_time));
                }
                if settings.Individual_level {
                    individualLvl();
                }
                isLoading();
                
                oldLevel = levelArray;
                sleep(Duration::from_nanos(16666667)).await;
            }
        }
    }).await;
}