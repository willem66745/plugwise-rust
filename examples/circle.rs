
extern crate getopts;
extern crate toml;

use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path;
use std::collections::HashMap;

use getopts::Options;

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

fn get_device_from_config<'a>(config: &'a toml::Table) -> Option<&'a str> {
    config.get(CONFIG_HEAD)
          .map_or(None, |item|item.as_table())
          .map_or(None, |table|table.get(CONFIG_DEVICE))
          .map_or(None, |string|string.as_str())
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

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();

    opts.optopt("s", "serial", "configure serial-port", "DEVICE")
        .optflag("t", "stub", "configure to use stub implementation")
        .optopt("a", "alias", "assign a alias to Mac", "NAME") // XXX
        .optflag("u", "unalias", "forget alias") // XXX
        .optflag("l", "list", "list aliassed circles") // XXX
        .optflag("r", "relaystatus", "print the relay status of a circle") // XXX
        .optflag("e", "enable", "enable the relay of a circle") // XXX
        .optflag("d", "disable", "disable the relay of a circle") // XXX
        .optflag("p", "powerusage", "print the actual power usage of a circle") // XXX
        .optopt("o", "powersince", "print the total power usage of a given number of days", "DAYS") // XXX
        .optflag("c", "clock", "print the internal clock value of a circle") // XXX
        .optflag("j", "updateclock", "update the internal clock of a circle using Internet time") // XXX
        .optflag("h", "help", "print this help menu") // XXX
        .optflag("v", "verbose", "print debug information"); // XXX

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

    println!("{:?}", serial); // FIXME: remove this
    // FIXME: implement more options

    if update_config {
        write_config(&configfile, &config);
    }

}
