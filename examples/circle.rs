
extern crate getopts;
extern crate ntpclient;
extern crate time;
extern crate toml;
extern crate plugwise;

use std::env;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path;
use std::collections::HashMap;

use getopts::Options;

use time::Duration;

use plugwise::Device;
use plugwise::ProtocolSnoop;
use plugwise::plugwise;

const CONFIG: &'static str = ".plugwise.toml";
const CONFIG_HEAD: &'static str = "config";
const CONFIG_DEVICE: &'static str = "device";
const ALIAS_MAC: &'static str = "mac";

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options] [mac|alias]", program);
    println!("{}", opts.usage(&brief));
}

fn load_config(configfile: &path::PathBuf) -> toml::Table {
    let mut config = String::new();
    if let Ok(mut file) = File::open(configfile) {
        file.read_to_string(&mut config).ok().expect(
            &format!("unable to read file `{}`", configfile.display()));
    } // errors are silently ignored (assuming file didn't exist)
    toml::Parser::new(&config).parse().expect(&format!("unable to parse `{}`", configfile.display()))
}

fn write_config(configfile: &path::PathBuf, config: &toml::Table) {
    let mut file = File::create(configfile).ok().expect(
        &format!("unable to create `{}`", configfile.display()));
    write!(file, "{}", toml::Value::Table(config.clone())).ok().expect(
        &format!("unable to write to `{}`", configfile.display()));
}

fn get_device_from_config<'a>(config: &'a toml::Table) -> Option<String> {
    config.get(CONFIG_HEAD)
          .map_or(None, |item|item.as_table())
          .map_or(None, |table|table.get(CONFIG_DEVICE))
          .map_or(None, |string|string.as_str())
          .map(|string|string.to_string())
}

fn get_aliases<'a>(config: &'a toml::Table) -> HashMap<String, u64> {
    let mut aliases = HashMap::new();

    for key in config.keys().filter(|&k|k != CONFIG_HEAD) {
        let mac = config.get(key)
                        .map_or(None, |item|item.as_table())
                        .map_or(None, |table|table.get(ALIAS_MAC))
                        .map_or(None, |string|string.as_str());
        if let Some(mac) = mac {
            if mac.len() == 16 {
                let mac = u64::from_str_radix(&mac, 16);
                if let Ok(mac) = mac {
                    aliases.insert(key.to_string(), mac);
                }
            }
        }
    }

    aliases
}

fn update_device_from_config<'a>(config: &'a toml::Table, device: &'a str) -> toml::Table {
    let mut config_table = config.get(CONFIG_HEAD)
                                 .map_or(None, |item|item.as_table())
                                 .map_or(toml::Table::new(), |table|table.clone());
    config_table.insert(CONFIG_DEVICE.to_string(), toml::Value::String(device.to_string()));
    config_table
}

fn remove_device_from_config<'a>(config: &'a toml::Table) -> toml::Table {
    let mut config_table = config.get(CONFIG_HEAD)
                                 .map_or(None, |item|item.as_table())
                                 .map_or(toml::Table::new(), |table|table.clone());
    config_table.remove(CONFIG_DEVICE);
    config_table
}

fn update_mac_in_alias<'a>(config: &'a toml::Table, alias: &'a str, mac: u64) -> toml::Table {
    let mut config_table = config.get(alias)
                                 .map_or(None, |item|item.as_table())
                                 .map_or(toml::Table::new(), |table|table.clone());
    config_table.insert(ALIAS_MAC.to_string(), toml::Value::String(format!("{:016X}", mac)));
    config_table
}

fn plugwise_actions(matches: &getopts::Matches, serial: Option<String>, mac: u64) {
    let mut debug = io::stdout();
    let snoop = match matches.opt_count("v") {
        0 => ProtocolSnoop::Nothing,
        1 => ProtocolSnoop::Debug(&mut debug),
        2 => ProtocolSnoop::Raw(&mut debug),
        _ => ProtocolSnoop::All(&mut debug)
    };
    let device = match serial {
        Some(ref serial) => Device::SerialExt{port: &serial,
                                              timeout: Duration::milliseconds(1000),
                                              retries: 3,
                                              snoop: snoop},
        None => Device::Simulator
    };
    if serial.is_none() {
        println!("WARNING: no serial device is specified to control the Plugwise hardware.");
        println!("         use option -s to specified the TTY/COM device. A simulated");
        println!("         version of the device is now used for testing purposes.");
        println!("");
    }

    let plugwise = plugwise(device).ok().expect("unable to connect to Plugwise device");
    let circle = plugwise.create_circle(mac).ok().expect("unable to connect to circle");

    if matches.opt_present("r") {
        let status = circle.is_switched_on().ok().expect("unable retrieve circle status");
        println!("circle {:016X} relay_status: {}", mac, status);
    } else if matches.opt_present("e") {
        circle.switch_on().ok().expect("unable to switch on circle");
        println!("circle {:016X} switched on", mac);
    } else if matches.opt_present("d") {
        circle.switch_off().ok().expect("unable to switch on circle");
        println!("circle {:016X} switched off", mac);
    } else if matches.opt_present("p") {
        let watts = circle.get_actual_watt_usage().ok()
                                                  .expect("unable to retrieve actual power usage");
        println!("circle {:016X} actual supplied power is: {} W", mac, watts);
    } else if let Some(days) = matches.opt_str("o") {
        let days = u32::from_str_radix(&days, 10).ok()
            .expect("provided number of days must be a positive decimal number");
        let period =  Duration::days(days as i64);
        let entries = Some(period.num_hours() as u32); // power usage entries are stored per hour

        let buffer = circle.get_power_buffer(entries).ok()
            .expect("unable to retrieve power usage history");

        if let Some(last_timestamp) = buffer.keys().last() {
            let kws = buffer.iter()
                            .filter(|&(&k, _)| (*last_timestamp - k) < period)
                            .fold(0.0, |acc, (_, &v)| acc + v);
            println!("circle {:016X} power usage last {} days is: {} kWh", mac, days, kws);
        } else {
            println!("circle {:016X} has no power usage history", mac);
        }
    } else if matches.opt_present("c") {
        let clock = circle.get_clock().ok().expect("unable to retrieve time from circle");
        println!("circle {:016X} time is: {} (UTC)", mac, clock.asctime());
    } else if matches.opt_present("j") {
        println!("retrieve time from the Internet...");
        let time = ntpclient::retrieve_ntp_timestamp("pool.ntp.org").ok()
            .expect("unable to retrieve timestamp");
        let tm = time::at_utc(time);
        println!("actual Internet time: {} (UTC)", tm.asctime());
        circle.set_clock(tm).ok().expect("unable to program time to circle");
        println!("circle {:016X} time has been updated", mac);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();

    opts.optopt("s", "serial", "configure serial-port", "DEVICE")
        .optflag("t", "stub", "configure to use stub implementation")
        .optopt("a", "alias", "assign a alias to Mac", "NAME")
        .optflag("u", "unalias", "forget alias")
        .optflag("l", "list", "list aliassed circles")
        .optflag("r", "relaystatus", "print the relay status of a circle")
        .optflag("e", "enable", "enable the relay of a circle")
        .optflag("d", "disable", "disable the relay of a circle")
        .optflag("p", "powerusage", "print the actual power usage of a circle")
        .optopt("o", "powersince", "print the total power usage of a given number of days", "DAYS")
        .optflag("c", "clock", "print the internal clock value of a circle")
        .optflag("j", "updateclock", "update the internal clock of a circle using Internet time")
        .optflag("h", "help", "print this help menu")
        .optflagmulti("v", "verbose", "print debug information");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m },
        Err(f) => { panic!(f.to_string()) }
    };

    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    }

    let mut configfile = env::home_dir().expect("unable to find home/user directory");
    configfile.push(CONFIG);

    let mut config = load_config(&configfile);
    let mut update_config = false;

    if let Some(new_device) = matches.opt_str("s") {
        // client has provided new device; update (any) loaded configuration
        let new_config = update_device_from_config(&config, &new_device);
        config.insert(CONFIG_HEAD.to_string(), toml::Value::Table(new_config));
        update_config = true;
    } else if matches.opt_present("t") {
        // client has indicated to use stub
        let new_config = remove_device_from_config(&config);
        config.insert(CONFIG_HEAD.to_string(), toml::Value::Table(new_config));
        update_config = true;
    }

    let serial = get_device_from_config(&config);
    let aliases = get_aliases(&config);
    let mac;

    if matches.opt_present("l") {
        for (alias, mac) in aliases {
            println!("{} (mac:{:016X})", alias, mac);
        }
        return;
    }

    // at least alias or mac must be specified
    if matches.free.len() == 1 {
        let free = &matches.free[0];
        // find mac by alias or try to decode mac address (16 digit hex)
        mac = aliases.get(free).map_or_else(|| {
            match free.len() {
                16 => u64::from_str_radix(free, 16).ok(),
                _ => None,
            }
        }, |&x| Some(x)).expect("unknown alias or MAC specified");

        if let Some(new_alias) = matches.opt_str("a") {
            let new_config = update_mac_in_alias(&config, &new_alias, mac);
            config.insert(new_alias, toml::Value::Table(new_config));
            update_config = true;
        } else if matches.opt_present("u") {
            config.remove(free).expect("cannot unalias MAC");
            update_config = true;
        } else {
            plugwise_actions(&matches, serial, mac);
        }
    } else if !update_config {
        panic!("only one alias or MAC must be specified");
    }

    if update_config {
        write_config(&configfile, &config);
    }
}
