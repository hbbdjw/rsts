#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod msr;

use eframe::egui;
use sysinfo::System;
use msr::WinRing0;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use std::thread;

const MSR_TURBO_RATIO_LIMIT: u32 = 0x1AD;

#[derive(PartialEq, Clone, Copy)]
enum PowerMode {
    Quiet,
    Balance,
    Beast,
}

struct App {
    system: Arc<Mutex<System>>,
    core_ratios: [u64; 8],
    fan_speed: u32,
    power_mode: PowerMode,
    msr_driver: Option<WinRing0>,
    error_msg: Option<String>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // sysinfo needs to be configured with specific flags to attempt to get dynamic frequency
        let mut sys = System::new_with_specifics(
            sysinfo::RefreshKind::nothing().with_cpu(sysinfo::CpuRefreshKind::everything()),
        );
        sys.refresh_cpu_specifics(sysinfo::CpuRefreshKind::everything());
        let system = Arc::new(Mutex::new(sys));

        // Background thread to refresh CPU periodically
        let sys_clone = Arc::clone(&system);
        let ctx = cc.egui_ctx.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(1000));
                if let Ok(mut s) = sys_clone.lock() {
                    s.refresh_cpu_specifics(sysinfo::CpuRefreshKind::everything());
                }
                ctx.request_repaint();
            }
        });

        let msr_driver = WinRing0::new().ok();
        
        let mut core_ratios = [48, 48, 48, 48, 46, 46, 46, 46];
        if let Some(msr) = &msr_driver {
            if let Some(val) = msr.read_msr(MSR_TURBO_RATIO_LIMIT) {
                for i in 0..8 {
                    core_ratios[i] = (val >> (i * 8)) & 0xFF;
                }
            }
        }

        Self {
            system,
            core_ratios,
            fan_speed: 50,
            power_mode: PowerMode::Balance,
            msr_driver,
            error_msg: None,
        }
    }
    
    fn apply_ratios(&mut self) {
        if let Some(msr) = &self.msr_driver {
            let mut ratio_value: u64 = 0;
            for i in 0..8 {
                ratio_value |= (self.core_ratios[i] & 0xFF) << (i * 8);
            }
            if !msr.write_msr(MSR_TURBO_RATIO_LIMIT, ratio_value) {
                self.error_msg = Some("Failed to write MSR 0x1AD. Make sure you run as Administrator.".to_string());
            } else {
                self.error_msg = Some("CPU Multipliers applied successfully!".to_string());
            }
        } else {
            // Attempt to re-init on click just in case
            match WinRing0::new() {
                Ok(msr) => {
                    self.msr_driver = Some(msr);
                    self.apply_ratios();
                }
                Err(e) => {
                    self.error_msg = Some(format!("WinRing0 init failed: {}", e));
                }
            }
        }
    }
    
    fn apply_power_mode(&mut self) {
        // Attempt to switch modes using WMI (this is a best-effort approach for Lenovo Legion)
        // Lenovo Legion typically uses ACPI/WMI to switch thermal modes. 
        // This is a complex area often requiring reverse-engineered WMI calls (e.g. root\wmi -> Lenovo_SetBiosSetting or GameZone)
        // Since we don't have the exact undocumented method, we simulate it or leave a placeholder.
        let mode_str = match self.power_mode {
            PowerMode::Quiet => "Quiet",
            PowerMode::Balance => "Balance",
            PowerMode::Beast => "Beast",
        };
        
        // Example WMI call (placeholder):
        // use wmi::{WMIConnection, COMLibrary};
        // if let Ok(com_con) = COMLibrary::new() {
        //     if let Ok(wmi_con) = WMIConnection::with_namespace_path("ROOT\\WMI", com_con) {
        //         // wmi_con.exec_query(...) or call method
        //     }
        // }

        self.error_msg = Some(format!("Switched to {} Mode (Hardware ACPI/WMI call requires Lenovo specific driver or exact WMI method)", mode_str));
    }
    
    fn apply_fan_speed(&mut self) {
        // Fan speed on Legion Y9000P is controlled via Embedded Controller (EC) registers.
        // It requires writing specific bytes to EC or using an undocumented WMI method.
        self.error_msg = Some(format!("Fan speed set to {}% (Requires direct EC write for actual hardware control)", self.fan_speed));
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Legion Y9000P Control Center");
            
            if self.msr_driver.is_none() {
                ui.colored_label(egui::Color32::RED, "Warning: WinRing0 Driver not loaded.");
                if let Some(err) = &self.error_msg {
                    ui.colored_label(egui::Color32::RED, err);
                }
            }

            ui.separator();
            
            ui.heading("Physical Core Frequencies (GHz)");
            
            egui::Grid::new("cpu_freq_grid")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .show(ui, |ui| {
                    let mut current_freq = 2.3; // Default fallback
                    
                    if let Ok(sys) = self.system.lock() {
                        if let Some(cpu) = sys.cpus().first() {
                            current_freq = cpu.frequency() as f32 / 1000.0;
                        }
                    }

                    // Try reading dynamic frequency from MSR 0x198 (IA32_PERF_STATUS)
                    // if WinRing0 is available.
                    if let Some(msr) = &self.msr_driver {
                        if let Some(perf_status) = msr.read_msr(0x198) {
                            let current_ratio = (perf_status >> 8) & 0xFF;
                            if current_ratio > 0 {
                                current_freq = current_ratio as f32 * 0.1;
                            }
                        }
                    }

                    for i in 0..8 {
                        ui.label(format!("Core {}: {:.2} GHz", i, current_freq));
                        
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });

            ui.separator();

            ui.heading("CPU Multiplier (Turbo Ratio Limit)");
            ui.vertical(|ui| {
                for i in 0..8 {
                    ui.add(egui::Slider::new(&mut self.core_ratios[i], 8..=60).text(format!("Core {}", i)));
                }
            });
            if ui.button("Apply CPU Multipliers").clicked() {
                self.apply_ratios();
            }

            ui.separator();

            ui.heading("Thermal Mode");
            ui.horizontal(|ui| {
                if ui.radio_value(&mut self.power_mode, PowerMode::Quiet, "Quiet Mode").clicked() {
                    self.apply_power_mode();
                }
                if ui.radio_value(&mut self.power_mode, PowerMode::Balance, "Balance Mode").clicked() {
                    self.apply_power_mode();
                }
                if ui.radio_value(&mut self.power_mode, PowerMode::Beast, "Beast Mode").clicked() {
                    self.apply_power_mode();
                }
            });

            ui.separator();

            ui.heading("Fan Speed Control");
            ui.add(egui::Slider::new(&mut self.fan_speed, 0..=100).text("% Speed"));
            if ui.button("Apply Fan Speed").clicked() {
                self.apply_fan_speed();
            }
            
            if let Some(msg) = &self.error_msg {
                ui.separator();
                ui.label(msg);
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([600.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Legion Y9000P Control Center",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
