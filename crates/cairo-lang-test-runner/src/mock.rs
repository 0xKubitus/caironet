///! Contract address mocking utils.
///!
///! Those utils are based on a simple JSON file to mock
///! contract addresses.
///!
///! For now, the file is expected to be named `.caironet.json`.

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::io::Read;

use serde::Deserialize;
use serde::Deserializer;
use serde_json::Value;

use num_bigint::BigUint;

use cairo_felt::Felt252;
use cairo_lang_runner::StarknetState;
use cairo_lang_starknet::contract::ContractInfo;

/// Mock configuration for a contract address.
#[derive(Debug)]
pub enum MockConfig {
    /// Only one address is represented by a single string.
    /// Example: "Contract1": "0x1".
    SingletonAddress(String),

    /// If several instance of the same contract may be deployed,
    /// using an object in the JSON is the way to go:
    /// "ERC20": { "Starkgate": "0x1234", "MyERC20": "0x98" }
    InstanceAddresses(HashMap<String, String>),
}

/// Custom deserialization for MockConfig to have a flat representation
/// of the expected fields.
impl<'de> Deserialize<'de> for MockConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value: Value = Deserialize::deserialize(deserializer)?;

        match value {
            Value::String(address) => Ok(MockConfig::SingletonAddress(address)),
            Value::Object(addresses) => {
                let map = addresses
                    .into_iter()
                    .map(|(k, v)| match v {
                        Value::String(value) => Ok((k, value)),
                        _ => Err(serde::de::Error::custom("Invalid value in InstanceAddresses")),
                    })
                    .collect::<Result<HashMap<String, String>, _>>()?;

                Ok(MockConfig::InstanceAddresses(map))
            }
            _ => Err(serde::de::Error::custom("Invalid MockConfig value")),
        }
    }
}

/// Parses JSON configuration file with mocked contract addresses.
///
/// * `path` - root path where the `.caironet.json` file is supposed to be found.
pub fn mocked_addresses_parse(
    path: &Path,
) -> anyhow::Result<HashMap<String, MockConfig>> {
    let path = path.join(".caironet.json");

    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => {
            println!("No contract address to be mocked as .caironet.json file was not found.");
            println!("Please check the README for the configuration.\n");
            return Ok(Default::default());
        }
    };

    let mut json = String::new();
    let content: HashMap<String, MockConfig> = match file.read_to_string(&mut json) {
        Ok(_) => match serde_json::from_str(&json) {
            Ok(c) => c,
            Err(e) => {
                println!("{:?}", e);
                anyhow::bail!("Mocked addressess file is not in the expected format.")
            }
        },
        Err(_) => anyhow::bail!("Mocked addresses file is not readable.")
    };

    Ok(content)
}

/// Builds a mocked `StarknetState` from mocked addresses.
///
/// * `mocked_addresses` - Mocked addresses from the JSON file.
/// * `contracts_info` - Contracts info collected by the compiler.
pub fn starknet_state_from_mocked_addresses(
    mocked_addresses: &HashMap<String, MockConfig>,
    contracts_info: &HashMap<Felt252, ContractInfo>,
) -> anyhow::Result<StarknetState> {

    let mut state: StarknetState = Default::default();

    for (class_hash, info) in contracts_info {
        if let Some(contract_name) = contract_name_from_info(info) {
            if let Some(mocked_addr) = mocked_addresses.get(contract_name) {
                match mocked_addr {
                    MockConfig::SingletonAddress(address) => {
                        println!("Mocked address: {} for {} (class_hash: {})",
                                 address,
                                 contract_name,
                                 class_hash);

                        state.contract_address_set(address_from_string(address), class_hash.clone());
                    },
                    MockConfig::InstanceAddresses(addresses) => {
                        for (instance_name, address) in addresses {
                            println!("Mocked address: {} for {} [{}] (class_hash: {})",
                                     address,
                                     contract_name,
                                     instance_name,
                                     class_hash);

                            state.contract_address_set(address_from_string(address), class_hash.clone());
                        }
                    }
                }
            }
        }
    }

    Ok(state.clone())
}

/// Gets a contract name from it's info in the contracts info.
/// If not found, return None.
///
/// This function assumes that a contracts always have at least one of:
/// 1. Constructor defined.
/// 2. One external function.
///
/// * `info` - Contracts info from the compiler parsing of the Sierra code with debug names.
fn contract_name_from_info(
    info: &ContractInfo,
) -> Option<&str> {
    match &info.constructor {
        Some(func_id) => {
            if let Some(debug_name) = &func_id.debug_name {
                let frags: Vec<&str> = debug_name.split("::").collect();
                Some(frags[frags.len() - 3])
            } else {
                None
            }
        },
        None => {
            if let Some(ext) = &info.externals.values().next() {
                if let Some(debug_name) = &ext.debug_name {
                    let frags: Vec<&str> = debug_name.split("::").collect();
                    Some(frags[frags.len() - 3])
                } else {
                    None
                }
            } else {
                None
            }
        }
    }
}

/// Converts an address string (dec or hex) into a Felt252.
///
/// * `addr` - Decimal or Hexadecimal string with the address to mock.
fn address_from_string(
    addr: &String,
) -> Felt252 {
    let mut hex_or_dec_addr: String = addr.clone();
    let mut radix: u32 = 10;
    if addr.starts_with("0x") {
        radix = 16;
        hex_or_dec_addr = hex_or_dec_addr.strip_prefix("0x").unwrap_or(addr).to_string();
    }

    let u = BigUint::parse_bytes(hex_or_dec_addr.as_bytes(), radix)
        .unwrap_or_else(|| panic!("Failed to parse BigUint from string '{}' with radix {}.", addr, radix));

    Felt252::from(u)
}
