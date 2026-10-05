//! CPU/GPU model detection. Port of `Backend/DetectHardware.py`.

use crate::wmi::Wmi;

/// Indices used by `Thermal`: fan 0 is the CPU fan, fan 1 the GPU fan.
pub const CPU_FAN_INDEX: usize = 0;
pub const GPU_FAN_INDEX: usize = 1;

const CIMV2_NAMESPACE: &str = "ROOT\\CIMV2";

pub struct DetectHardware {
    wmi: Wmi,
}

impl DetectHardware {
    pub fn connect() -> Option<DetectHardware> {
        Wmi::connect(CIMV2_NAMESPACE)
            .ok()
            .map(|wmi| DetectHardware { wmi })
    }

    pub fn cpu_name(&self) -> Option<String> {
        let processors = self.wmi.query("SELECT Name FROM Win32_Processor")?;
        let name = processors.first()?.get_string("Name")?;
        let trimmed = name.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }

    /// Picks the adapter with the most memory, which is the discrete GPU on the
    /// laptops this app targets.
    pub fn gpu_name(&self) -> Option<String> {
        let adapters = self
            .wmi
            .query("SELECT Name, AdapterRAM FROM Win32_VideoController")?;
        let best = adapters
            .iter()
            .max_by_key(|adapter| adapter.get_u32("AdapterRAM").unwrap_or(0))?;
        let name = best.get_string("Name")?;
        let trimmed = name.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }

    /// Name of the hardware behind `fan_index`, mirroring the Python API.
    pub fn hardware_name(&self, fan_index: usize) -> Option<String> {
        match fan_index {
            CPU_FAN_INDEX => self.cpu_name(),
            GPU_FAN_INDEX => self.gpu_name(),
            _ => None,
        }
    }
}
