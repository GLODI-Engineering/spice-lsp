//! Which SPICE dialect a document is being parsed as, and the per-dialect
//! device-letter overlay table.
//!
//! See `docs/GRAMMAR.md` §9.1: dialect must be explicit (pinned per file
//! or workspace), never silently sniffed — same-letter device collisions
//! (`P`, `U`) and operator-meaning flips (`^`, `log()`, handled elsewhere
//! in [`crate::expr`]) make full auto-detection unreliable in the general
//! case.

use std::fmt;

/// Which SPICE-family dialect a document should be parsed/interpreted as.
/// Every entry point in this crate that isn't dialect-agnostic-by-design
/// takes one of these explicitly — nothing in this crate tries to guess it
/// from file content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    /// [ngspice](https://ngspice.sourceforge.io/), the open-source SPICE3
    /// derivative.
    Ngspice,
    /// [Xyce](https://xyce.sandia.gov/), Sandia National Laboratories'
    /// parallel circuit simulator.
    Xyce,
}

impl Dialect {
    /// Resolve a device-instance letter (the first character of an
    /// element-instance line, e.g. `'R'` in `R1 1 2 100`) to its meaning
    /// under this dialect, or `None` if the letter isn't a recognized
    /// device type here.
    ///
    /// This is the seam where same-letter collisions between dialects are
    /// resolved: `'P'` and `'U'` mean different devices in ngspice vs.
    /// Xyce (see `docs/GRAMMAR.md` §3.2) — always go through this method
    /// rather than assuming a device-letter table is shared across
    /// dialects.
    ///
    /// ```
    /// use spice_core::dialect::{Dialect, DeviceKind};
    ///
    /// assert_eq!(
    ///     Dialect::Ngspice.resolve_device_letter('P'),
    ///     Some(DeviceKind::CoupledMulticonductorLine)
    /// );
    /// assert_eq!(
    ///     Dialect::Xyce.resolve_device_letter('P'),
    ///     Some(DeviceKind::PortDevice)
    /// );
    /// ```
    pub fn resolve_device_letter(self, letter: char) -> Option<DeviceKind> {
        match self {
            Dialect::Ngspice => ngspice_device_letter(letter),
            Dialect::Xyce => xyce_device_letter(letter),
        }
    }
}

/// What kind of device an [`crate::ast::ElementInstance`]'s device letter
/// resolved to, under a specific [`Dialect`] (via
/// [`Dialect::resolve_device_letter`]). Used by the parser to decide how
/// many leading tokens are nodes vs. parameters (see
/// [`DeviceKind::min_nodes`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    /// Resistor (`R`).
    Resistor,
    /// Capacitor (`C`).
    Capacitor,
    /// Inductor (`L`).
    Inductor,
    /// Coupled/mutual inductor (`K`).
    MutualInductor,
    /// Independent voltage source (`V`).
    VoltageSource,
    /// Independent current source (`I`).
    CurrentSource,
    /// Diode (`D`).
    Diode,
    /// Bipolar junction transistor (`Q`).
    Bjt,
    /// Junction field-effect transistor (`J`).
    Jfet,
    /// MESFET (`Z`).
    Mesfet,
    /// MOSFET (`M`).
    Mosfet,
    /// Voltage-controlled voltage source (`E`) — the classic 4-node form;
    /// see `docs/GRAMMAR.md` §5.2 for the alternate 2-node behavioral
    /// (`VALUE=`/`TABLE`/`POLY`) form both dialects also accept.
    Vcvs,
    /// Voltage-controlled current source (`G`) — same 4-node/2-node
    /// distinction as [`DeviceKind::Vcvs`].
    Vccs,
    /// Current-controlled current source (`F`).
    Cccs,
    /// Current-controlled voltage source (`H`).
    Ccvs,
    /// Behavioral (arbitrary) source (`B`).
    Behavioral,
    /// Voltage-controlled switch (`S`).
    VoltageSwitch,
    /// Current-controlled switch (`W`).
    CurrentSwitch,
    /// Lossless transmission line (`T`).
    TransmissionLine,
    /// Subcircuit call (`X`) — references a [`crate::ast::Subckt`]
    /// definition by name.
    SubcircuitCall,
    /// ngspice-only: XSPICE code model (`A`).
    XspiceCodeModel,
    /// ngspice-only: Verilog-A/OSDI compact model (`N`).
    VerilogACompactModel,
    /// Lossy transmission line (`O` in both dialects).
    LossyTransmissionLine,
    /// ngspice-only: uniform distributed RC line (`U` under ngspice — note
    /// this is a different device from Xyce's `U`, see
    /// [`DeviceKind::BehavioralDigital`]).
    UniformRCLine,
    /// ngspice-only: coupled multiconductor line (`P` under ngspice — a
    /// different device from Xyce's `P`, see [`DeviceKind::PortDevice`]).
    CoupledMulticonductorLine,
    /// ngspice-only: single lossy transmission line (`Y`).
    SingleLossyLine,
    /// Xyce-only: S-parameter port device (`P` under Xyce — a different
    /// device from ngspice's `P`, see
    /// [`DeviceKind::CoupledMulticonductorLine`]).
    PortDevice,
    /// Xyce-only: behavioral digital device (`U` under Xyce — a different
    /// device from ngspice's `U`, see [`DeviceKind::UniformRCLine`]).
    BehavioralDigital,
    /// Xyce-only: accelerated-mass device (`YACC`).
    YAcc,
    /// Xyce-only: ideal delay device (`YDELAY`).
    YDelay,
    /// Xyce-only: linear S/Y/Z-parameter device (`YLIN`).
    YLin,
    /// Xyce-only: memristor device (`YMEMRISTOR`).
    YMemristor,
    /// Xyce-only: lumped transmission line (`ytransline`).
    YTransline,
    /// Xyce-only: PDE/TCAD device (`YPDE`).
    YPde,
}

impl fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl DeviceKind {
    /// The minimum number of leading tokens on an element-instance line
    /// that count as node names, before parameters start. For device
    /// kinds where this isn't a fixed, universal number (see
    /// [`DeviceKind::is_fixed_node_count`] — e.g. a subcircuit call's node
    /// count depends on the referenced `.subckt`'s own declaration), this
    /// returns a permissive lower bound the parser uses as a starting
    /// point, not a hard maximum.
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

    /// Whether [`DeviceKind::min_nodes`] is the device's exact, fixed node
    /// count (`true`) or just a lower bound because the real count varies
    /// by instance — e.g. a subcircuit call's node count depends on the
    /// referenced `.subckt`, a polynomial source's on its declared degree
    /// (`false` for both).
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
