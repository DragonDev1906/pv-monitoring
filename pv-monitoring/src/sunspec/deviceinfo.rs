//! Gather useful static fields from the sunspec data (module 1)
use super::Field;

#[derive(Debug, Clone, Default)]
pub struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub options: String,
    pub sn: String,
}

pub fn parse_mod1(fields: &[Field]) -> Option<DeviceInfo> {
    if fields.len() != 6 {
        eprintln!("WARN: Module 1 has unexpected number of fields");
        return None;
    }
    let expected_fields = fields[0].point.name == "Mn"
        && fields[1].point.name == "Md"
        && fields[2].point.name == "Opt"
        && fields[3].point.name == "Vr"
        && fields[4].point.name == "SN"
        && fields[5].point.name == "DA"
        && fields[5].value.len() == 1;

    if !expected_fields {
        eprintln!("WARN: Module 1 has unexpected fields");
        return None;
    }

    Some(DeviceInfo {
        manufacturer: super::parse_string(&fields[0].value),
        model: super::parse_string(&fields[1].value),
        options: super::parse_string(&fields[2].value),
        sn: super::parse_string(&fields[4].value),
    })
}
