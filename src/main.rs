mod sunspec;
mod telegraf;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args();
    let _ = args.next().unwrap();
    let addr = args.next().unwrap();
    assert_eq!(args.next(), None);

    // let (addr, port): (String, u16) = addr.split_once(':').map(|a, p| (a, p.parse())).unwrap_or((addr, 502));
    let (addr, port) = match addr.split_once(':') {
        Some((addr, port)) => (addr, port.parse()?),
        None => (addr.as_str(), 1502),
    };

    const BASE_ADDR: u16 = 40000;
    const DEVICE_ID: u8 = 1;

    let cfg = modbus::Config {
        tcp_port: port,
        tcp_connect_timeout: None,
        tcp_read_timeout: None,
        tcp_write_timeout: None,
        modbus_uid: DEVICE_ID,
    };
    let mut client = modbus::Transport::new_with_cfg(addr, cfg)?;

    let out = sunspec::collect(&mut client, DEVICE_ID, BASE_ADDR)?;

    eprintln!();
    for block in &out.blocks {
        telegraf::write_config(std::io::stdout(), block)?;
    }

    client.close()?;
    Ok(())
}

/*
fn main() {
    let mut args = std::env::args();
    let _ = args.next();
    let module_id: usize = args.next().unwrap().parse().unwrap();
    let output = args.next().unwrap();
    assert_eq!(args.next(), None);

    let module: Module = reqwest::blocking::get(url(module_id)).unwrap().json().unwrap();
    let mut holding_registers: Vec<&Point> = vec![];
    let mut input_registers: Vec<&Point> = vec![];

    // Group into telegraf buckets (best effort)
    for p in &module.group.points {
        use PointType as T;
        if p.writable {
            holding_registers.push(p);
        } else {
            input_registers.push(p);
        }
    }

    // Output the buckets
    let mut f = std::fs::File::create(output).unwrap();

    if !holding_registers.is_empty() {
        write!(f, "[[inputs.modbus.request]]").unwrap();
        write!(f, "slave_id = 1").unwrap();
        write!(f, "byte_order = \"ABCD\"").unwrap();
        write!(f, "register = \"coil\"").unwrap();
        write!(f, "fields = [").unwrap();
        for p in input_registers {
            match p.typ {

            }
            // write!(f, "  {{ address={}, name={},  }}").unwrap();
            // write!(f, "  {{ address={}, name={}, type={}, bit={}, scale={} }}").unwrap();
        }
        write!(f, "]").unwrap();
    }

    if !module.group.groups.is_empty() {
        println!("Module contains {}nested groups (not yet supported)", module.group.groups.len());
    }
}
*/
