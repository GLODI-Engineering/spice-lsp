use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    Ngspice,
    Xyce,
}

impl Dialect {
    pub fn resolve_device_letter(self, letter: char) -> Option<DeviceKind> {
        match self {
            Dialect::Ngspice => ngspice_device_letter(letter),
            Dialect::Xyce => xyce_device_letter(letter),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Resistor,
    Capacitor,
    Inductor,
    MutualInductor,
    VoltageSource,
    CurrentSource,
    Diode,
    Bjt,
    Jfet,
    Mesfet,
    Mosfet,
    Vcvs,
    Vccs,
    Cccs,
    Ccvs,
    Behavioral,
    VoltageSwitch,
    CurrentSwitch,
    TransmissionLine,
    SubcircuitCall,
    XspiceCodeModel,
    VerilogACompactModel,
    LossyTransmissionLine,
    UniformRCLine,
    CoupledMulticonductorLine,
    SingleLossyLine,
    PortDevice,
    BehavioralDigital,
    YAcc,
    YDelay,
    YLin,
    YMemristor,
    YTransline,
    YPde,
}

impl fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl DeviceKind {
    pub fn min_nodes(self) -> usize {
        match self {
            DeviceKind::Resistor
            | DeviceKind::Capacitor
            | DeviceKind::Inductor
            | DeviceKind::VoltageSource
            | DeviceKind::CurrentSource
            | DeviceKind::Diode
            | DeviceKind::Behavioral
            | DeviceKind::PortDevice => 2,
            DeviceKind::Bjt | DeviceKind::Jfet | DeviceKind::Mesfet => 3,
            DeviceKind::Mosfet
            | DeviceKind::Vcvs
            | DeviceKind::Vccs
            | DeviceKind::VoltageSwitch => 4,
            DeviceKind::TransmissionLine => 4,
            DeviceKind::LossyTransmissionLine | DeviceKind::UniformRCLine => 3,
            _ => 1,
        }
    }

    pub fn is_fixed_node_count(self) -> bool {
        !matches!(
            self,
            DeviceKind::SubcircuitCall
                | DeviceKind::MutualInductor
                | DeviceKind::Cccs
                | DeviceKind::Ccvs
                | DeviceKind::CurrentSwitch
                | DeviceKind::XspiceCodeModel
                | DeviceKind::VerilogACompactModel
                | DeviceKind::CoupledMulticonductorLine
                | DeviceKind::SingleLossyLine
                | DeviceKind::BehavioralDigital
                | DeviceKind::YAcc
                | DeviceKind::YDelay
                | DeviceKind::YLin
                | DeviceKind::YMemristor
                | DeviceKind::YTransline
                | DeviceKind::YPde
        )
    }
}

fn ngspice_device_letter(letter: char) -> Option<DeviceKind> {
    match letter {
        'R' => Some(DeviceKind::Resistor),
        'C' => Some(DeviceKind::Capacitor),
        'L' => Some(DeviceKind::Inductor),
        'K' => Some(DeviceKind::MutualInductor),
        'V' => Some(DeviceKind::VoltageSource),
        'I' => Some(DeviceKind::CurrentSource),
        'D' => Some(DeviceKind::Diode),
        'Q' => Some(DeviceKind::Bjt),
        'J' => Some(DeviceKind::Jfet),
        'Z' => Some(DeviceKind::Mesfet),
        'M' => Some(DeviceKind::Mosfet),
        'E' => Some(DeviceKind::Vcvs),
        'G' => Some(DeviceKind::Vccs),
        'F' => Some(DeviceKind::Cccs),
        'H' => Some(DeviceKind::Ccvs),
        'B' => Some(DeviceKind::Behavioral),
        'S' => Some(DeviceKind::VoltageSwitch),
        'W' => Some(DeviceKind::CurrentSwitch),
        'T' => Some(DeviceKind::TransmissionLine),
        'X' => Some(DeviceKind::SubcircuitCall),
        'A' => Some(DeviceKind::XspiceCodeModel),
        'N' => Some(DeviceKind::VerilogACompactModel),
        'O' => Some(DeviceKind::LossyTransmissionLine),
        'U' => Some(DeviceKind::UniformRCLine),
        'P' => Some(DeviceKind::CoupledMulticonductorLine),
        'Y' => Some(DeviceKind::SingleLossyLine),
        _ => None,
    }
}

fn xyce_device_letter(letter: char) -> Option<DeviceKind> {
    match letter {
        'R' => Some(DeviceKind::Resistor),
        'C' => Some(DeviceKind::Capacitor),
        'L' => Some(DeviceKind::Inductor),
        'K' => Some(DeviceKind::MutualInductor),
        'V' => Some(DeviceKind::VoltageSource),
        'I' => Some(DeviceKind::CurrentSource),
        'D' => Some(DeviceKind::Diode),
        'Q' => Some(DeviceKind::Bjt),
        'J' => Some(DeviceKind::Jfet),
        'Z' => Some(DeviceKind::Mesfet),
        'M' => Some(DeviceKind::Mosfet),
        'E' => Some(DeviceKind::Vcvs),
        'G' => Some(DeviceKind::Vccs),
        'F' => Some(DeviceKind::Cccs),
        'H' => Some(DeviceKind::Ccvs),
        'B' => Some(DeviceKind::Behavioral),
        'S' => Some(DeviceKind::VoltageSwitch),
        'W' => Some(DeviceKind::CurrentSwitch),
        'T' => Some(DeviceKind::TransmissionLine),
        'X' => Some(DeviceKind::SubcircuitCall),
        'O' => Some(DeviceKind::LossyTransmissionLine),
        'U' => Some(DeviceKind::BehavioralDigital),
        'P' => Some(DeviceKind::PortDevice),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_letter_p_collision_resolves_differently_per_dialect() {
        assert_eq!(
            Dialect::Ngspice.resolve_device_letter('P'),
            Some(DeviceKind::CoupledMulticonductorLine)
        );
        assert_eq!(
            Dialect::Xyce.resolve_device_letter('P'),
            Some(DeviceKind::PortDevice)
        );
        assert_ne!(
            Dialect::Ngspice.resolve_device_letter('P'),
            Dialect::Xyce.resolve_device_letter('P')
        );
    }

    #[test]
    fn test_device_letter_u_collision_resolves_differently_per_dialect() {
        assert_eq!(
            Dialect::Ngspice.resolve_device_letter('U'),
            Some(DeviceKind::UniformRCLine)
        );
        assert_eq!(
            Dialect::Xyce.resolve_device_letter('U'),
            Some(DeviceKind::BehavioralDigital)
        );
        assert_ne!(
            Dialect::Ngspice.resolve_device_letter('U'),
            Dialect::Xyce.resolve_device_letter('U')
        );
    }

    #[test]
    fn test_unknown_device_letter_returns_explicit_none_not_panic() {
        assert_eq!(Dialect::Ngspice.resolve_device_letter('1'), None);
        assert_eq!(Dialect::Ngspice.resolve_device_letter('!'), None);
        assert_eq!(Dialect::Xyce.resolve_device_letter('1'), None);
        assert_eq!(Dialect::Xyce.resolve_device_letter('A'), None);
    }

    #[test]
    fn test_common_device_letters_agree_across_dialects() {
        let common_letters = [
            'R', 'C', 'L', 'K', 'V', 'I', 'D', 'Q', 'J', 'Z', 'M', 'E', 'G', 'F', 'H', 'B', 'S',
            'W', 'T', 'X', 'O',
        ];
        for &letter in &common_letters {
            let ng = Dialect::Ngspice.resolve_device_letter(letter);
            let xy = Dialect::Xyce.resolve_device_letter(letter);
            assert_eq!(
                ng, xy,
                "device letter '{letter}' diverged: ngspice={ng:?}, xyce={xy:?}"
            );
        }
    }
}
