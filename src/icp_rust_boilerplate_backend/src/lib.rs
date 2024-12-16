#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

// Define a struct for storing energy usage details
#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct EnergyUsage {
    id: u64,                     // Unique identifier for each record
    usage_kwh: f64,              // Energy usage in kilowatt-hours
    timestamp: u64,              // Time of the recorded usage (in nanoseconds since epoch)
    device_type: String,         // Type of device consuming the energy
    recommendation: Option<String>, // Optional energy-saving recommendation
}

// Implement the Storable trait for EnergyUsage
impl Storable for EnergyUsage {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

// Implement the BoundedStorable trait for EnergyUsage
impl BoundedStorable for EnergyUsage {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

// Thread-local storage for memory management and data storage
thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static STORAGE: RefCell<StableBTreeMap<u64, EnergyUsage, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
        ));
}

// Struct for energy usage payload from users
#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct EnergyUsagePayload {
    usage_kwh: f64,              // Energy usage in kilowatt-hours
    device_type: String,         // Type of device consuming the energy
}

// Add a new energy usage record
#[ic_cdk::update]
fn add_energy_usage(payload: EnergyUsagePayload) -> Result<EnergyUsage, Error> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment ID counter");

    let energy_usage = EnergyUsage {
        id,
        usage_kwh: payload.usage_kwh,
        timestamp: time(),
        device_type: payload.device_type,
        recommendation: Some(generate_recommendation(payload.usage_kwh)),
    };

    do_insert(&energy_usage)?;
    Ok(energy_usage)
}

// Insert the energy usage record into storage
fn do_insert(energy_usage: &EnergyUsage) -> Result<(), Error> {
    STORAGE.with(|service| {
        service.borrow_mut().insert(energy_usage.id, energy_usage.clone())
    });
    Ok(())
}

// Generate energy-saving recommendations based on usage
fn generate_recommendation(usage_kwh: f64) -> String {
    if usage_kwh > 10.0 {
        "High energy usage detected. Consider reducing the number of devices or optimizing usage.".to_string()
    } else if usage_kwh > 5.0 {
        "Moderate energy usage. Consider using energy-efficient devices.".to_string()
    } else {
        "Low energy usage. Keep up the good work!".to_string()
    }
}

// Retrieve an energy usage record by ID
#[ic_cdk::query]
fn get_energy_usage(id: u64) -> Result<EnergyUsage, Error> {
    match _get_energy_usage(&id) {
        Some(usage) => Ok(usage),
        None => Err(Error::NotFound {
            msg: format!("Energy usage record with ID {} not found", id),
        }),
    }
}

// Helper method to fetch the energy usage record from storage
fn _get_energy_usage(id: &u64) -> Option<EnergyUsage> {
    STORAGE.with(|s| s.borrow().get(id))
}

// Delete an energy usage record by ID
#[ic_cdk::update]
fn delete_energy_usage(id: u64) -> Result<EnergyUsage, Error> {
    match STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(usage) => Ok(usage),
        None => Err(Error::NotFound {
            msg: format!("Energy usage record with ID {} not found.", id),
        }),
    }
}

// Define error types for the canister
#[derive(candid::CandidType, Deserialize, Serialize, Debug)]
enum Error {
    NotFound { msg: String },    // Record not found
    MemoryFull { msg: String },  // Storage limit reached
    InvalidInput { msg: String }, // Invalid input provided
}

// Export the Candid interface for the canister
ic_cdk::export_candid!();

// Integration tests (to be run locally or with CI tools)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_energy_usage() {
        let payload = EnergyUsagePayload {
            usage_kwh: 12.0,
            device_type: "Air Conditioner".to_string(),
        };
        let record = add_energy_usage(payload).unwrap();
        assert_eq!(record.usage_kwh, 12.0);
        assert!(get_energy_usage(record.id).is_ok());
    }

    #[test]
    fn test_delete_energy_usage() {
        let payload = EnergyUsagePayload {
            usage_kwh: 5.0,
            device_type: "Laptop".to_string(),
        };
        let record = add_energy_usage(payload).unwrap();
        assert!(delete_energy_usage(record.id).is_ok());
        assert!(get_energy_usage(record.id).is_err());
    }
}
