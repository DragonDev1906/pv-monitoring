//! Code for outputting in the telegraf config format
use std::io::Write;

use anyhow::Result;

use crate::sunspec::{Block, IntOrString, PointType, State};

/// Write a telegraf configuration for the given fields.
///
/// - Expects the fields to be in a reasonable order (ideally sorted by address but not required)
/// - Expects all fields to be accessible as (read-only) registers.
pub fn write_config(mut f: impl Write, block: &Block) -> Result<()> {
    write!(
        f,
        "    [[inputs.modbus.metric]] # Sunspec module {}",
        block.module_id
    )?;
    if let Some(s) = &block.group.label {
        writeln!(f, " {}", s)?;
    } else {
        writeln!(f)?;
    }
    writeln!(f, "    slave_id = {}", block.device_id)?;
    writeln!(f, "    byte_order = \"ABCD\"")?;
    writeln!(
        f,
        "    measurement = \"sunspec_{}_{}\"",
        block.module_id, block.group.name
    )?;
    writeln!(f, "    fields = [")?;
    let mut wants_empty_line = false;
    let mut first = true;
    for e in &block.fields {
        // No use fetching and storing fields that are not
        // changing (at least not regularly).
        // The below fields are marked static in sunspec because they rarely change,
        // but they still make sense as fields in influx/telegraf.
        let include = !e.point.is_static
            || (block.module_id == 1 && e.point.name == "Vr")
            || e.state == State::SFUsedByHasValue;
        let commented = !include || e.state == State::ProbablyNotImplemented;

        // If we have a comment before the line: Separate it from other lines
        wants_empty_line |= match e.point.typ {
            PointType::Enum16 | PointType::Enum32 => true,
            PointType::Bitfield16 | PointType::Bitfield32 | PointType::Bitfield64 => {
                !e.point.symbols.is_empty()
            }
            _ => false,
        };

        // Use empty lines to clearly indicate where a comment belongs to.
        if wants_empty_line && !first {
            writeln!(f, "")?;
            wants_empty_line = false;
        }

        first = false;

        // Comment
        if let PointType::Enum16 | PointType::Enum32 = e.point.typ {
            write!(
                f,
                "        # Enum {}: ",
                e.point
                    .label
                    .as_ref()
                    .or(e.point.desc.as_ref())
                    .unwrap_or(&e.point.name)
            )?;
            for sym in &e.point.symbols {
                let serde_json::Value::Number(value) = &sym.value else {
                    continue;
                };
                write!(f, "{}:{}, ", value, sym.name)?;
            }
            writeln!(f, "")?;
            wants_empty_line = true;
        }

        // Main line
        if commented {
            write!(f, "        # ")?;
        } else {
            write!(f, "        ")?;
        }
        let typ = type_ident(e.point.typ);
        write!(
            f,
            "{{ address={}, name=\"{}\", type=\"{}\"",
            e.addr, e.point.name, typ
        )?;
        match e.point.typ {
            PointType::String => write!(f, ", length={}", e.point.size)?,
            _ => {}
        }
        if let Some(IntOrString::Int(sf)) = e.point.sf {
            write!(f, ", scale={}", sf)?;
        }
        let inline_comment = matches!(e.point.sf, Some(IntOrString::String(_)))
            || e.point.units.is_some()
            || e.point.label.is_some()
            || e.point.desc.is_some();
        if inline_comment {
            write!(f, " }}, \t#")?;
            if let Some(IntOrString::String(sf)) = &e.point.sf {
                write!(f, " Scale: 10^{{{}}},", sf)?;
            }
            if let Some(units) = &e.point.units {
                write!(f, " Unit: {},", units)?;
            }
            if let Some(s) = &e.point.label {
                write!(f, " {},", s)?;
            }
            if let Some(s) = &e.point.desc {
                write!(f, " {},", s)?;
            }
            if e.point.is_static {
                write!(f, " (static)")?;
            }
            writeln!(f, "")?;
        } else {
            writeln!(f, " }},")?;
        }

        // Alternative bit expansion
        if let PointType::Bitfield16 | PointType::Bitfield32 | PointType::Bitfield64 = e.point.typ {
            for sym in &e.point.symbols {
                let serde_json::Value::Number(bit) = &sym.value else {
                    continue;
                };
                writeln!(
                    f,
                    "        # {{ address={}, name=\"{}_{}\", type=\"BIT\", bit={} }}",
                    e.addr, e.point.name, sym.name, bit,
                )?;
            }

            wants_empty_line = !e.point.symbols.is_empty();
        }
    }
    writeln!(f, "    ]\n")?;

    // Telegraf apparently does not support the dotted keys syntax of toml.
    writeln!(f, "        [inputs.modbus.metric.tags]")?;
    writeln!(
        f,
        "        manufacturer = \"{}\"",
        block.device_info.manufacturer
    )?;
    writeln!(f, "        model = \"{}\"", block.device_info.model)?;
    writeln!(f, "        options = \"{}\"", block.device_info.options)?;
    writeln!(f, "        sn = \"{}\"", block.device_info.sn)?;
    writeln!(f, "\n\n")?;

    Ok(())
}

fn type_ident(typ: PointType) -> &'static str {
    // BIT (single bit of a register)
    // INT8L, INT8H, UINT8L, UINT8H (low and high byte variants)
    // INT16, UINT16, INT32, UINT32, INT64, UINT64 and
    // FLOAT16, FLOAT32, FLOAT64 (IEEE 754 binary representation)
    // STRING (byte-sequence converted to string)
    use PointType as T;
    match typ {
        T::Int16 | T::Sunssf => "INT16",
        T::Int32 => "INT32",
        T::Int64 => "INT64",
        T::Uint16 | T::Acc16 | T::Bitfield16 | T::Enum16 | T::Raw16 | T::Count => "UINT16",
        T::Uint32 | T::Acc32 | T::Bitfield32 | T::Enum32 => "UINT32",
        T::Uint64 | T::Acc64 | T::Bitfield64 => "UINT64",
        T::Float32 => "FLOAT32",
        T::Float64 => "FLOAT64",
        T::String => "STRING",
        T::Pad => "UINT16", // Should not be recorded, but just in case
        T::Ipaddr | T::Ipv6addr | T::Eui48 => unimplemented!(),
    }
}
