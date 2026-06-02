use std::collections::HashMap;

use anyhow::Result;
use modbus::{Client, Transport};

mod json_model;
pub use json_model::{Group, IntOrString, Module, Point, PointType};

mod deviceinfo;
pub use deviceinfo::DeviceInfo;

fn url(module: u16) -> String {
    format!(
        "https://github.com/sunspec/models/raw/refs/heads/master/json/model_{}.json",
        module
    )
}

#[derive(Debug)]
pub struct Output {
    pub blocks: Vec<Block>,
    #[allow(unused)]
    pub len: u16,
}

#[derive(Debug)]
pub struct Block {
    pub device_id: u8,
    pub module_id: usize,
    pub group: Group,
    pub fields: Vec<Field>,
    pub device_info: DeviceInfo,
}

#[derive(Debug)]
pub struct Field {
    pub addr: u16,
    pub point: Point,
    pub value: Vec<u16>,
    pub state: State,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum State {
    ProbablyNotImplemented,
    HasValue,

    SFUsedByStaticOrNotImpl,
    SFUsedByHasValue,
}

pub fn collect(client: &mut Transport, device_id: u8, base_addr: u16) -> Result<Output> {
    let mut cache: HashMap<u16, json_model::Module> = HashMap::new();

    // "SunS" magic value
    let magic = client.read_holding_registers(base_addr, 2)?;
    assert_eq!(magic, [0x5375, 0x6e53], "Expected SunS constant");

    let mut current_device_info = DeviceInfo::default();
    let mut blocks = vec![];

    let mut haddr = base_addr + 2;
    loop {
        // Read the module/block ID and length
        let header = client.read_holding_registers(haddr, 2)?;
        debug_assert_eq!(header.len(), 2);
        let module_id = header[0];
        let len = header[1];

        // Stop when we find the end.
        if module_id == 0xffff {
            break;
        }

        // Get definition for this module/block.
        let def = match cache.get(&module_id) {
            Some(def) => def,
            None => {
                let module: Module = reqwest::blocking::get(url(module_id))?.json()?;
                cache.insert(module_id, module);
                &cache[&module_id]
            }
        };
        eprintln!(
            "\n{haddr}: Sunspec module {module_id} ({len} registers): {}",
            def.group.label.as_ref().unwrap_or(&def.group.name)
        );

        // Fetch the data (to know we can read it and to print it).
        // There seems to be a limit to how many values we can request at once,
        // but the rust library just hangs instead of returning an error.
        // So we request it in chunks.
        let values = {
            let mut l = 0;
            let mut values = vec![];
            while l < len {
                values.extend(client.read_holding_registers(haddr + 2 + l, u16::min(len, 100))?);
                l += 100;
            }
            values
        };
        // let values = client.read_holding_registers(haddr + 2, len)?;

        // Sanity check
        assert!(def.group.points.len() >= 2);
        assert_eq!(def.group.points[0].size, 1);
        assert_eq!(def.group.points[1].size, 1);
        assert_eq!(
            def.group.points[0].value,
            Some(IntOrString::Int(module_id.into()))
        );

        // Go through all registers (points), log their current value and add it to the lists.
        // dbg!(&values);
        let mut a = 0u16;
        let mut fields = vec![];
        for p in &def.group.points[2..] {
            // For some reason trailing padding fields are not honored.
            if a + p.size > values.len() as u16 {
                eprintln!(
                    "INFO: Block is missing field at the end: {} (type: {:?})",
                    &p.name, &p.typ
                );
                continue;
            }

            // Values shared between basically all match arms.
            let values = &values[usize::from(a)..(usize::from(a) + usize::from(p.size))];
            let addr = haddr + 2 + a;

            // Decide what to do
            use PointType as T;
            match p.typ {
                T::Int16 | T::Int32 | T::Int64 => print_int(addr, p, &values, None),
                // Raw16
                T::Uint16 | T::Uint32 | T::Uint64 | T::Acc16 | T::Acc32 | T::Acc64 => {
                    print_uint(addr, p, &values, None)
                }
                T::Bitfield16 | T::Bitfield32 | T::Bitfield64 => {
                    print_bitfield(addr, p, &values, None)
                }
                T::Enum16 => print_enum16(addr, p, values[0], None),
                // Enum3A, Float32, Float642
                T::String => print_string(addr, p, &values, None),
                T::Pad => {}
                // Ipaddr, Ipv6addr, Eui48
                T::Sunssf => print_int(addr, p, &values, Some("Scale Factor")),
                // Count,
                _ => print_field(addr, p, &values, None, not_implemented_heuristic(p, values)),
            }

            // Add all fields including static ones,
            // this way the main function or the config writer can decide what to do.
            fields.push(Field {
                addr,
                point: p.clone(),
                value: values.to_vec(),
                state: State::HasValue, // Heuristic runs in set_states.
            });

            a += p.size;
        }

        // TODO: Process nested groups (e.g. in 706)
        if !def.groups.is_empty() {
            eprintln!("WARN: Module contains nested groups (not implemented yet)");
        }

        if module_id == 1 {
            current_device_info = deviceinfo::parse_mod1(&fields).unwrap();
        }

        set_states(&mut fields);

        blocks.push(Block {
            device_id,
            module_id: def.id,
            // PERFORMANCE: We could probably strip the points and subgroups.
            group: def.group.clone(),
            fields,
            device_info: current_device_info.clone(),
        });
        haddr += 2 + len;
    }

    Ok(Output {
        blocks,
        len: haddr + 1 - base_addr,
    })
}

// Returns true if the value is likely not implemented
fn not_implemented_heuristic(p: &Point, value: &[u16]) -> bool {
    match p.typ {
        PointType::Acc32 if value[0] == 0 && value[1] == 0 && p.name.starts_with("Tot") => true,
        PointType::Sunssf | PointType::Int16 | PointType::Int32 | PointType::Int64
            if value[0] == 0x8000 =>
        {
            true
        }
        PointType::Uint16 if value[0] == 0xffff => true,
        PointType::Uint32 if value[0] == 0xffff && value[1] == 0xffff => true,
        PointType::Uint64
            if value[0] == 0xffff
                && value[1] == 0xffff
                && value[2] == 0xffff
                && value[3] == 0xffff =>
        {
            true
        }
        _ => false,
    }
}

// TODO: Missmatch with telegraf exporter: Static values are commented out but marked as
// HasValue here, resulting in a useless field entry by default.
fn set_states(fields: &mut [Field]) {
    // Reset all usage fields (heuristic for NotImplemented)
    for f in &mut *fields {
        if not_implemented_heuristic(&f.point, &f.value) {
            f.state = State::ProbablyNotImplemented;
        } else {
            f.state = State::HasValue;
        }
    }

    // Index based iteration works because we do not move anything around.
    // Iterator based iteration would result in borrow checker problems.
    for i in 0..fields.len() {
        // Fields without a SF are irrelevant, skip them.
        let Some(IntOrString::String(sf)) = &fields[i].point.sf else {
            continue;
        };

        // Copy to avoid borrow checker issues
        let sf = sf.clone();
        let state = fields[i].state;
        let is_static = fields[i].point.is_static;

        // Find the corresponding SF entry.
        let Some(sf) = fields.iter_mut().find(|f| f.point.name == *sf) else {
            continue;
        };

        // Mark it
        let mut set_ni = false; // Can't set it directly
        use State as S;
        sf.state = match (sf.state, state, is_static) {
            // A garbage/unused SF indicates that the field itself likely is not implemented either.
            (S::ProbablyNotImplemented, _, _) => {
                set_ni = true;
                S::ProbablyNotImplemented
            }

            // Keep or upgrade to UsedByHasValue
            (S::SFUsedByHasValue, _, _) => S::SFUsedByHasValue,
            (_, S::HasValue, _) => S::SFUsedByHasValue,

            // Keep or update to UsedByStaticOrNotImpl
            (S::SFUsedByStaticOrNotImpl, _, _) => S::SFUsedByStaticOrNotImpl,
            (_, S::ProbablyNotImplemented, _) => S::SFUsedByStaticOrNotImpl,
            (_, _, true) => S::SFUsedByStaticOrNotImpl,

            // Otherwise don't change the value.
            // NOTE: This also covers the case where usage is a SF_* value.
            (x, _, _) => x,
        };

        // Propagate invalid SF values back to the fields that use them.
        if set_ni {
            fields[i].state = S::ProbablyNotImplemented;
        }
    }
}

fn print_bitfield(addr: u16, p: &Point, value: &[u16], alt_label: Option<&str>) {
    let mut s = String::new();
    for sym in &p.symbols {
        let serde_json::Value::Number(ref sym_value) = sym.value else {
            eprintln!("WARN: Ignoring non-integer symbol value for bitfield");
            continue;
        };
        let sym_value = sym_value.as_u64().unwrap();
        let reg = (sym_value / 16) as usize;
        let bit = sym_value % 16;
        // Looks like 0 means true
        let set = (value[reg] >> bit) & 0x01 == 0;
        if set {
            if !s.is_empty() {
                s += "|";
            }
            s += &sym.name;
        }
    }
    let mut sum = 0;
    for v in value {
        sum <<= 16;
        sum |= *v as u64;
    }

    // Try to guess when the value is unsupported/not available.
    // For some reason I am getting 0 and 0xff for mandatory values with symbols.
    // And because it is very unlikely that every flag is set, we assume it is not implemented
    // properly or unused.
    if s.is_empty() || sum == 0 {
        print_field(
            addr,
            p,
            format!("(0x{sum:x})"),
            alt_label,
            not_implemented_heuristic(p, value),
        );
    } else {
        print_field(
            addr,
            p,
            format!("{s} (0x{sum:x})"),
            alt_label,
            not_implemented_heuristic(p, value),
        );
    }
}
fn print_enum16(addr: u16, p: &Point, value: u16, alt_label: Option<&str>) {
    let sym = p.symbols.iter().find(|s| s.value == value);
    match sym {
        None => print_field(addr, p, value, alt_label, value == 0xffff),
        Some(sym) => {
            let value_str = format!("{} ({})", sym.name, sym.value);
            print_field(addr, p, value_str, alt_label, false);
        }
    }
}

fn print_int(addr: u16, p: &Point, value: &[u16], alt_label: Option<&str>) {
    assert!(!value.is_empty());
    assert!(value.len() <= 4);
    // Take the sign bit and start with 0x00 or 0xff
    let sign_bit = (value[0] >> 15) & 0x1;
    let mut sum = -(sign_bit as i64);
    // The fist value already has the correct bit.
    for v in value {
        sum <<= 16;
        sum |= *v as i64;
    }

    print_field(addr, p, sum, alt_label, not_implemented_heuristic(p, value));
}
fn print_uint(addr: u16, p: &Point, value: &[u16], alt_label: Option<&str>) {
    let mut sum = 0;
    for v in value {
        sum <<= 16;
        sum |= *v as u64;
    }
    print_field(addr, p, sum, alt_label, not_implemented_heuristic(p, value));
}
fn parse_string(value: &[u16]) -> String {
    let bytes = value
        .iter()
        .flat_map(|reg| [(reg >> 8) as u8, (reg & 0xff) as u8])
        .take_while(|b| *b != 0)
        .collect();
    String::from_utf8(bytes).unwrap()
}
fn print_string(addr: u16, p: &Point, value: &[u16], alt_label: Option<&str>) {
    print_field(
        addr,
        p,
        parse_string(value),
        alt_label,
        not_implemented_heuristic(p, value),
    );
}

fn print_field(
    addr: u16,
    p: &Point,
    value: impl std::fmt::Debug,
    alt_label: Option<&str>,
    not_implemented_heuristic: bool,
) {
    if not_implemented_heuristic {
        eprint!("\x1b[90m");
    }
    eprint!(
        "{}: {:40} {:<16} = {:?}",
        addr,
        p.label.as_deref().or(alt_label).unwrap_or_default(),
        p.name,
        value
    );
    match &p.sf {
        None => {}
        Some(IntOrString::Int(sf)) => eprint!(" * {}", 10_f32.powi(*sf)),
        Some(IntOrString::String(sf)) => eprint!(" * 10^{{{sf}}}"),
    }

    let brackets = p.is_static || !p.mandatory || not_implemented_heuristic;
    if brackets {
        eprint!(" (");
        if p.is_static {
            eprint!("static,");
        }
        if !p.mandatory {
            eprint!("opt,");
        }
        if not_implemented_heuristic {
            eprint!("prob-not-impl");
        }
        eprintln!(")\x1b[0m");
    } else {
        eprintln!("\x1b[0m");
    }
}
