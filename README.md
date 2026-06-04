# PV Monitoring
Most PV systems and inverters push you towards their own cloud monitoring system. This project makes it easier to configure a local monitoring stack based on your PV setup.

It should work with all Inverters that implement the [Sunspec](https://sunspec.org/) and are reachable via modbus over TCP. An ESP that converts physical modbus to TCP should also work, a direct interaction with modbus hardware connected to this device is currently not implemented.

## Tested Inverters
- SolarEdge SE10K-RWB48BFN4 (default port 1502, can be enabled with Installer permissions)

## How it works
We connect via tcpmodbus to the Inverter and query all registers belonging to sunspec. The addresses, names and current values are printed (based on the sunspec schema). The exact sunspec layout can differ between hardware setups and potentially firmware updates, but monitoring applications are usually not aware of Sunspec. At the end we output the configuration file with hard coded register addresses, the dynamic registers enabled and the static registers commented out. It also contains information from the Sunspec schema to more easily see the meaning of these registers.

In addition, it tries to detect register values that are not implemented by your inverter and comment them out, too.

## Usage (Telegraf)
First: Connect to the inverter, query via modbus and generate the configuration file:

```bash
# IP and Port are from the device that exposes Sunspec modbus over TCP.
# NOTE: The cli options may change in future versions to allow more flexibility.
cargo run -- 192.168.178.42:1502 | tee telegraf-sunspec.toml
```

> [!info]
> The config file should be regenerated after hardware changes on the PV/Inverter setup, as that can change the register layout. It might also be necessary after firmware updates (less likely).

Then add it to your Telegraf configuration file:

```
[[inputs.modbus]]
  name = "Inverter"
  slave_id = 1 # Range: 0 - 255 [0 = broadcast; 248 - 255 = reserved]
  timeout = "1s"
  controller = "tcp://192.168.178.42:1502"
  configuration_type = "metric"

  # Add the content of telegraf.sunspec.toml here
```

> [!info] Suggestion
> I have tested this with Influxdb and Grafana.

## Roadmap
> [!info]
> These may or may not happen, but this is where this project might go.

- [ ] Grafana dashboard
- [ ] Register discovery outside of Sunspec, unfortunately that will not have documented meanings
- [ ] Improve CLI options
- [ ] Other config exporters (e.g. HomeAssistant)
- [ ] Maybe: Comparison against existing configuration to detect changes on which lines are commented out
- [ ] Maybe: Support for related device(s) like heat pumps or chargers.

