//! The part picker (`space`): one key per part, first level depends on the flavor;
//! submenus make every part reachable from every flavor.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Part(&'static str),
    Menu(&'static str),
    Presets,
    /// Block diagram node type.
    Block(&'static str),
    /// Magnetic figure item.
    Magnetic(&'static str),
    /// Turn an empty figure into a block diagram.
    ToBlock,
}

pub struct Entry {
    pub key: char,
    pub label: &'static str,
    pub action: Action,
}

const fn p(key: char, label: &'static str, kind: &'static str) -> Entry {
    Entry { key, label, action: Action::Part(kind) }
}
const fn m(key: char, label: &'static str, menu: &'static str) -> Entry {
    Entry { key, label, action: Action::Menu(menu) }
}

const SUBMENUS: &[Entry] = &[
    m('s', "sources …", "sources"),
    m('e', "electronics …", "electronics"),
    m('&', "logic …", "logic"),
    m('m', "meters …", "meters"),
    m('w', "switches & misc …", "misc"),
    m('v', "ground & supply …", "supply"),
    Entry { key: 'P', label: "presets …", action: Action::Presets },
];

pub fn menu(name: &str) -> Vec<&'static Entry> {
    static NETWORK: &[Entry] = &[
        p('r', "resistor", "resistor"),
        p('c', "capacitor", "capacitor"),
        p('l', "inductor", "inductor"),
        p('z', "impedance", "impedance"),
        p('u', "voltage source", "vsource"),
        p('i', "current source", "isource"),
        p('g', "ground", "ground"),
        p('n', "node dot", "node"),
        p('t', "terminal", "terminal"),
        p('b', "labeled block", "block"),
        p('T', "transformer", "transformer"),
        Entry { key: 'B', label: "turn into a block diagram", action: Action::ToBlock },
    ];
    static EQUIVALENT: &[Entry] = &[
        p('r', "resistor", "resistor"),
        p('c', "capacitor", "capacitor"),
        p('l', "inductor", "inductor"),
        p('z', "impedance", "impedance"),
        p('u', "voltage source", "vsource"),
        p('i', "current source", "isource"),
        p('U', "controlled voltage source", "dvsource"),
        p('I', "controlled current source", "disource"),
        p('g', "ground", "ground"),
        p('n', "node dot", "node"),
        p('t', "terminal", "terminal"),
        p('b', "labeled block", "block"),
        p('T', "transformer", "transformer"),
    ];
    static SCHEMATIC: &[Entry] = &[
        p('r', "resistor", "resistor"),
        p('c', "capacitor", "capacitor"),
        p('d', "diode", "diode"),
        p('n', "npn", "npn"),
        p('p', "pnp", "pnp"),
        p('f', "n-MOSFET", "nmos"),
        p('o', "op-amp", "opamp"),
        p('g', "ground", "ground"),
        p('t', "terminal", "terminal"),
        p('.', "node dot", "node"),
        p('b', "labeled block", "block"),
        p('T', "transformer", "transformer"),
    ];
    static LOGIC: &[Entry] = &[
        p('a', "AND", "land"),
        p('o', "OR", "lor"),
        p('x', "XOR", "lxor"),
        p('n', "NOT", "lnot"),
        p('A', "NAND", "lnand"),
        p('O', "NOR", "lnor"),
        p('X', "XNOR", "lxnor"),
        p('f', "D flip-flop", "dflipflop"),
        p('t', "terminal", "terminal"),
        p('.', "node dot", "node"),
        p('b', "labeled block", "block"),
    ];
    static SOURCES: &[Entry] = &[
        p('u', "voltage source", "vsource"),
        p('i', "current source", "isource"),
        p('a', "AC voltage source", "acvsource"),
        p('U', "controlled voltage source", "dvsource"),
        p('I', "controlled current source", "disource"),
        p('b', "battery", "battery"),
    ];
    static ELECTRONICS: &[Entry] = &[
        p('d', "diode", "diode"),
        p('z', "zener diode", "zener"),
        p('l', "LED", "led"),
        p('n', "npn", "npn"),
        p('p', "pnp", "pnp"),
        p('f', "n-MOSFET", "nmos"),
        p('F', "p-MOSFET", "pmos"),
        p('o', "op-amp", "opamp"),
    ];
    static LOGIC_ALL: &[Entry] = &[
        p('a', "AND", "land"),
        p('o', "OR", "lor"),
        p('x', "XOR", "lxor"),
        p('n', "NOT", "lnot"),
        p('A', "NAND", "lnand"),
        p('O', "NOR", "lnor"),
        p('X', "XNOR", "lxnor"),
        p('d', "D flip-flop", "dflipflop"),
        p('j', "JK flip-flop", "jkflipflop"),
        p('s', "SR latch", "srlatch"),
    ];
    static METERS: &[Entry] = &[p('v', "voltmeter", "voltmeter"), p('a', "ammeter", "ammeter")];
    static MISC: &[Entry] = &[
        p('s', "switch", "switch"),
        p('f', "fuse", "fuse"),
        p('l', "lamp", "lamp"),
        p('p', "potentiometer", "potentiometer"),
        p('c', "polarised capacitor", "pcapacitor"),
        p('b', "labeled block", "block"),
        p('T', "transformer", "transformer"),
    ];
    static SUPPLY: &[Entry] = &[
        p('g', "ground", "ground"),
        p('e', "earth", "earth"),
        p('+', "supply +", "vcc"),
        p('-', "supply −", "vee"),
    ];
    static BLOCK: &[Entry] = &[
        Entry { key: 's', label: "sum", action: Action::Block("sum") },
        Entry { key: 'k', label: "gain", action: Action::Block("gain") },
        Entry { key: 'i', label: "integrator", action: Action::Block("integrator") },
        Entry { key: 't', label: "transfer function G(s)", action: Action::Block("tf") },
        Entry { key: 'f', label: "function", action: Action::Block("function") },
        Entry { key: 'b', label: "branch point", action: Action::Block("branch") },
        Entry { key: 'e', label: "input / output", action: Action::Block("io") },
        Entry { key: 'P', label: "presets …", action: Action::Presets },
    ];
    static MAGNETIC: &[Entry] = &[
        Entry { key: 'w', label: "winding on the leg at the cursor", action: Action::Magnetic("winding") },
        Entry { key: 'f', label: "flux loop", action: Action::Magnetic("flux") },
        Entry { key: 'y', label: "flux arrow on the leg (Ψ)", action: Action::Magnetic("arrow") },
        Entry { key: 's', label: "cross-section", action: Action::Magnetic("section") },
        Entry { key: 'g', label: "air gap", action: Action::Magnetic("gap") },
        Entry { key: 'P', label: "presets …", action: Action::Presets },
    ];
    let first: &[Entry] = match name {
        "network" => NETWORK,
        "equivalent" => EQUIVALENT,
        "schematic" => SCHEMATIC,
        "logic" => LOGIC,
        "sources" => return SOURCES.iter().collect(),
        "electronics" => return ELECTRONICS.iter().collect(),
        "logic-all" | "logic …" => return LOGIC_ALL.iter().collect(),
        "meters" => return METERS.iter().collect(),
        "misc" => return MISC.iter().collect(),
        "supply" => return SUPPLY.iter().collect(),
        "block" => return BLOCK.iter().collect(),
        "magnetic" => return MAGNETIC.iter().collect(),
        _ => NETWORK,
    };
    // Submenus come after the flavor's own keys; a flavor key wins on a clash.
    let mut out: Vec<&Entry> = first.iter().collect();
    for s in SUBMENUS {
        if !out.iter().any(|e| e.key == s.key) {
            out.push(s);
        }
    }
    out
}

/// The `&` submenu is named "logic" in SUBMENUS but must not collide with the flavor.
pub fn resolve_menu(name: &str) -> &str {
    if name == "logic" { "logic-all" } else { name }
}
