//! Dell AWCC thermal interface.
//!
//! Port of `Backend/AWCCWmiWrapper.py` and `Backend/AWCCThermal.py`.

use crate::wmi::{Wmi, WmiError, WmiMethod};

pub const SENSOR_ID_FIRST: u32 = 0x01;
pub const SENSOR_ID_LAST: u32 = 0x30;
pub const FAN_ID_FIRST: u32 = 0x31;
pub const FAN_ID_LAST: u32 = 0x63;

/// Fan order reported by the driver: index 0 is the CPU fan, 1 the GPU fan.
pub const CPU_FAN_INDEX: usize = 0;
pub const GPU_FAN_INDEX: usize = 1;

const AWCC_CLASS: &str = "AWCCWmiMethodFunction";
const AWCC_NAMESPACE: &str = "ROOT\\WMI";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalMode {
    Custom,
    Balanced,
    GMode,
}

impl ThermalMode {
    pub const ALL: [ThermalMode; 3] = [
        ThermalMode::Balanced,
        ThermalMode::GMode,
        ThermalMode::Custom,
    ];

    pub fn value(self) -> u32 {
        match self {
            ThermalMode::Custom => 0x00,
            ThermalMode::Balanced => 0x97,
            ThermalMode::GMode => 0xAB,
        }
    }

    /// Settings-file spelling, matching the Python GUI (`"G_Mode"`).
    pub fn key(self) -> &'static str {
        match self {
            ThermalMode::Custom => "Custom",
            ThermalMode::Balanced => "Balanced",
            ThermalMode::GMode => "G_Mode",
        }
    }

    /// Text shown in the UI (`"G Mode"`).
    pub fn label(self) -> &'static str {
        match self {
            ThermalMode::Custom => "Custom",
            ThermalMode::Balanced => "Balanced",
            ThermalMode::GMode => "G Mode",
        }
    }

    pub fn from_key(key: &str) -> Option<ThermalMode> {
        ThermalMode::ALL.into_iter().find(|m| m.key() == key)
    }
}

#[derive(Debug)]
pub enum ThermalError {
    /// The AWCC WMI class is not present (drivers missing / unsupported model).
    NoWmiClass,
    /// The class exists but could not be used (usually: not running as admin).
    CannotInstantiate,
}

impl std::fmt::Display for ThermalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThermalError::NoWmiClass => write!(f, "AWCC WMI class not found in the system"),
            ThermalError::CannotInstantiate => {
                write!(f, "Couldn't instantiate AWCC WMI class")
            }
        }
    }
}

impl std::error::Error for ThermalError {}

const USTT_BALANCED: u32 = 0xA0;

/// Thin, stateless wrapper over the `AWCCWmiMethodFunction` methods.
pub struct AwccWmi {
    wmi: Wmi,
    /// Object path used to invoke methods, e.g. `AWCCWmiMethodFunction`.
    target: String,
    thermal_information: WmiMethod,
    thermal_control: WmiMethod,
    get_fan_sensors: WmiMethod,
    /// Number of instances the `AWCCWmiMethodFunction` query returned.
    instance_count: usize,
    /// Cached result of the USTT-balanced probe; `None` until first needed.
    balanced_mode_patch: Option<Option<u32>>,
}

impl AwccWmi {
    pub fn connect() -> Result<AwccWmi, ThermalError> {
        let wmi = Wmi::connect(AWCC_NAMESPACE).map_err(|_| ThermalError::CannotInstantiate)?;

        let instances = wmi
            .query(&format!("SELECT * FROM {AWCC_CLASS}"))
            .ok_or(ThermalError::NoWmiClass)?;
        let target = instances
            .first()
            .and_then(|instance| instance.get_string("__PATH"))
            .unwrap_or_else(|| AWCC_CLASS.to_string());

        let thermal_information = wmi
            .method(AWCC_CLASS, "Thermal_Information")
            .ok_or(ThermalError::CannotInstantiate)?;
        let thermal_control = wmi
            .method(AWCC_CLASS, "Thermal_Control")
            .ok_or(ThermalError::CannotInstantiate)?;
        let get_fan_sensors = wmi
            .method(AWCC_CLASS, "GetFanSensors")
            .ok_or(ThermalError::CannotInstantiate)?;

        Ok(AwccWmi {
            wmi,
            target,
            thermal_information,
            thermal_control,
            get_fan_sensors,
            instance_count: instances.len(),
            balanced_mode_patch: None,
        })
    }

    /// Object path methods are invoked on.
    pub fn target_path(&self) -> &str {
        &self.target
    }

    /// Raw results of the read-only methods, for troubleshooting.
    pub fn diagnose(&self) -> Vec<String> {
        let describe = |result: Result<u32, WmiError>| match result {
            Ok(value) => format!("ok ({value})"),
            Err(error) => format!("failed: {error}"),
        };
        let first_fan = FAN_ID_FIRST;
        vec![
            format!("WMI instances found: {}", self.instance_count),
            format!("object path: {}", self.target),
            format!(
                "declared input parameters: Thermal_Information='{}', Thermal_Control='{}', GetFanSensors='{}'",
                self.thermal_information.argument_name(),
                self.thermal_control.argument_name(),
                self.get_fan_sensors.argument_name()
            ),
            format!(
                "Thermal_Information(fan 0x{first_fan:02X}, rpm): {}",
                describe(self.call_result(&self.thermal_information, (first_fan << 8) | 5))
            ),
            format!(
                "GetFanSensors(fan 0x{first_fan:02X}, count): {}",
                describe(self.call_result(&self.get_fan_sensors, (first_fan << 8) | 1))
            ),
            format!(
                "Thermal_Information(sensor 0x{SENSOR_ID_FIRST:02X}, temp): {}",
                describe(
                    self.call_result(&self.thermal_information, (SENSOR_ID_FIRST << 8) | 4)
                )
            ),
        ]
    }

    fn is_sensor_id(id: u32) -> bool {
        (SENSOR_ID_FIRST..=SENSOR_ID_LAST).contains(&id)
    }

    fn is_fan_id(id: u32) -> bool {
        (FAN_ID_FIRST..=FAN_ID_LAST).contains(&id)
    }

    fn call(&self, method: &WmiMethod, arg: u32) -> Option<u32> {
        method.call(&self.wmi, &self.target, arg)
    }

    fn call_result(&self, method: &WmiMethod, arg: u32) -> Result<u32, WmiError> {
        method.exec(&self.wmi, &self.target, arg)
    }

    pub fn sensor_temperature(&self, sensor_id: u32) -> Option<u32> {
        if !Self::is_sensor_id(sensor_id) {
            return None;
        }
        self.call(&self.thermal_information, (sensor_id << 8) | 4)
    }

    pub fn fan_rpm_percent(&self, fan_id: u32) -> Option<u32> {
        if !Self::is_fan_id(fan_id) {
            return None;
        }
        self.call(&self.thermal_information, (fan_id << 8) | 6)
    }

    pub fn fan_rpm(&self, fan_id: u32) -> Option<u32> {
        if !Self::is_fan_id(fan_id) {
            return None;
        }
        self.call(&self.thermal_information, (fan_id << 8) | 5)
    }

    pub fn fan_related_sensors_count(&self, fan_id: u32) -> Option<u32> {
        if !Self::is_fan_id(fan_id) {
            return None;
        }
        self.call(&self.get_fan_sensors, (fan_id << 8) | 1)
    }

    pub fn fan_related_sensor(&self, fan_id: u32, sensor_index: u32) -> Option<u32> {
        if !Self::is_fan_id(fan_id) {
            return None;
        }
        self.call(
            &self.get_fan_sensors,
            (sensor_index << 16) | (fan_id << 8) | 2,
        )
    }

    /// `(fan id, related sensor ids)` for every fan the driver reports.
    pub fn fan_ids_and_related_sensors(&self) -> Vec<(u32, Vec<u32>)> {
        let mut result = Vec::new();
        for fan_id in FAN_ID_FIRST..=FAN_ID_LAST {
            let count = self.fan_related_sensors_count(fan_id).unwrap_or(0);
            if count == 0 {
                continue;
            }
            let sensors = (0..count)
                .filter_map(|index| self.fan_related_sensor(fan_id, index))
                .collect();
            result.push((fan_id, sensors));
        }
        result
    }

    fn thermal_control(&self, arg: u32) -> bool {
        self.apply_mode_value(arg).ok() == Some(0)
    }

    /// Applies a raw thermal-mode constant. Success is `Ok(0)`.
    pub fn apply_mode_value(&self, mode: u32) -> Result<u32, WmiError> {
        self.call_result(&self.thermal_control, (mode & 0xFF) << 8 | 1)
    }

    /// Invokes a named AWCC method with one `uint32` argument.
    pub fn call_named(&self, name: &str, arg: u32) -> Result<u32, WmiError> {
        let method = self.wmi.method(AWCC_CLASS, name).ok_or(WmiError {
            context: "method not present",
            hr: 0,
        })?;
        method.exec(&self.wmi, &self.target, arg)
    }

    pub fn apply_thermal_mode(&mut self, mode: ThermalMode) -> bool {
        let mut value = mode.value();
        if mode == ThermalMode::Balanced {
            // Laptops with USTT support reject the regular balanced value and
            // want 0xA0 instead; probe once and remember the answer.
            if self.balanced_mode_patch.is_none() {
                let patch = if self.thermal_control(USTT_BALANCED) {
                    Some(USTT_BALANCED)
                } else {
                    None
                };
                self.balanced_mode_patch = Some(patch);
            }
            if let Some(Some(patch)) = self.balanced_mode_patch {
                value = patch;
            }
        }
        self.thermal_control(value)
    }

    pub fn set_fan_speed_percent(&self, fan_id: u32, speed: u32) -> bool {
        if !Self::is_fan_id(fan_id) {
            return false;
        }
        let speed = speed.min(0xFF);
        let arg = (speed << 16) | (fan_id << 8) | 2;
        self.call(&self.thermal_control, arg) == Some(0)
    }
}

/// Thermal interface with the fan/sensor layout already resolved.
pub struct Thermal {
    awcc: AwccWmi,
    fans: Vec<(u32, Vec<u32>)>,
    sensor_ids: Vec<u32>,
}

impl Thermal {
    /// Connects and resolves the fan layout.
    ///
    /// An empty fan list means the driver refused to talk to us - in practice
    /// that is the "start me as administrator" case.
    pub fn connect() -> Result<Thermal, ThermalError> {
        let thermal = Thermal::from_awcc(AwccWmi::connect()?);
        if thermal.fans.is_empty() {
            return Err(ThermalError::CannotInstantiate);
        }
        Ok(thermal)
    }

    pub fn from_awcc(awcc: AwccWmi) -> Thermal {
        let fans = awcc.fan_ids_and_related_sensors();
        let sensor_ids = fans
            .iter()
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect();
        Thermal {
            awcc,
            fans,
            sensor_ids,
        }
    }

    /// Fan id / related sensor ids, in driver order (CPU first, then GPU).
    pub fn fans(&self) -> &[(u32, Vec<u32>)] {
        &self.fans
    }

    pub fn awcc(&self) -> &AwccWmi {
        &self.awcc
    }

    pub fn fan_count(&self) -> usize {
        self.fans.len()
    }

    pub fn all_temps(&self) -> Vec<Option<u32>> {
        self.sensor_ids
            .iter()
            .map(|id| self.awcc.sensor_temperature(*id))
            .collect()
    }

    pub fn all_fan_rpm(&self) -> Vec<Option<u32>> {
        self.fans
            .iter()
            .map(|(id, _)| self.awcc.fan_rpm(*id))
            .collect()
    }

    pub fn set_all_fan_speed(&self, speed: u32) -> bool {
        self.fans
            .iter()
            .map(|(id, _)| self.awcc.set_fan_speed_percent(*id, speed))
            .fold(true, |acc, ok| acc & ok)
    }

    pub fn fan_related_temp(&self, fan_index: usize) -> Option<u32> {
        let (_, sensors) = self.fans.get(fan_index)?;
        self.awcc.sensor_temperature(*sensors.first()?)
    }

    pub fn fan_rpm(&self, fan_index: usize) -> Option<u32> {
        let (fan_id, _) = self.fans.get(fan_index)?;
        self.awcc.fan_rpm(*fan_id)
    }

    pub fn set_fan_speed(&self, fan_index: usize, speed: u32) -> bool {
        match self.fans.get(fan_index) {
            Some((fan_id, _)) => self.awcc.set_fan_speed_percent(*fan_id, speed),
            None => false,
        }
    }

    pub fn set_mode(&mut self, mode: ThermalMode) -> bool {
        self.awcc.apply_thermal_mode(mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_keys_round_trip() {
        for mode in ThermalMode::ALL {
            assert_eq!(ThermalMode::from_key(mode.key()), Some(mode));
        }
        assert_eq!(ThermalMode::GMode.key(), "G_Mode");
        assert_eq!(ThermalMode::GMode.label(), "G Mode");
        assert_eq!(ThermalMode::from_key("nonsense"), None);
    }

    #[test]
    fn mode_values_match_driver_constants() {
        assert_eq!(ThermalMode::Custom.value(), 0x00);
        assert_eq!(ThermalMode::Balanced.value(), 0x97);
        assert_eq!(ThermalMode::GMode.value(), 0xAB);
    }

    #[test]
    fn argument_encoding_matches_documented_layout() {
        // From WMI-AWCC-doc.md.
        let sensor_id = 0x12u32;
        assert_eq!((sensor_id << 8) | 4, 0x1204);
        let fan_id = 0x32u32;
        assert_eq!((fan_id << 8) | 5, 0x3205);
        let speed = 0x40u32;
        assert_eq!((speed << 16) | (fan_id << 8) | 2, 0x403_202);
    }
}
