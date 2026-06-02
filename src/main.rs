mod sunspec;
mod telegraf;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args();
    let _ = args.next().unwrap();
    let addr = args.next().unwrap();
    assert_eq!(args.next(), None);

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
