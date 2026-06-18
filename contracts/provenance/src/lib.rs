#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracterror, contracttype, log,
    Address, Env, String, Vec,
};

// ========================
// Error types
// ========================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyRegistered = 1,
    NotAuthorized = 2,
    NotFound = 3,
    AlreadyExists = 4,
    BadRequest = 5,
    WrongRole = 6,
}

// ========================
// Data types
// ========================

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Role {
    Farmer,
    Processor,
    Shipper,
    Retailer,
    Inspector,
    Other,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductStatus {
    Created,
    InTransit,
    Processed,
    Inspected,
    Delivered,
    Recalled,
    Archived,
}

#[contracttype]
#[derive(Clone)]
pub struct Participant {
    pub address: Address,
    pub role: Role,
    pub name: String,
    pub metadata: String,
    pub registered_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct Product {
    pub id: u64,
    pub name: String,
    pub origin: String,
    pub owner: Address,
    pub status: ProductStatus,
    pub metadata: String,
    pub created_at: u64,
    pub updated_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct Event {
    pub id: u64,
    pub product_id: u64,
    pub event_type: String,
    pub location: String,
    pub actor: Address,
    pub metadata: String,
    pub timestamp: u64,
}

// ========================
// Storage keys
// ========================

#[contracttype]
pub enum DataKey {
    Participant(Address),
    Product(u64),
    Event(u64, u64),
    ProductEvents(u64, u32),
    ProductCount,
    EventCount(u64),
    ProductList(u32),
    ProductListLen,
    Admin,
}

// ========================
// Helper traits
// ========================

trait Saveable: Clone {
    fn save(&self, env: &Env, key: DataKey);
    fn load(env: &Env, key: &DataKey) -> Option<Self>;
}

impl<T: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + soroban_sdk::TryFromVal<Env, soroban_sdk::Val> + Clone>
    Saveable for T
{
    fn save(&self, env: &Env, key: DataKey) {
        env.storage().persistent().set(&key, self);
    }

    fn load(env: &Env, key: &DataKey) -> Option<Self> {
        env.storage().persistent().get(key)
    }
}

// ========================
// Contract
// ========================

#[contract]
pub struct Provenance;

#[contractimpl]
impl Provenance {
    // ---------- Initialization ----------

    pub fn init(env: Env, admin: Address) {
        admin.require_auth();
        let existing: Option<Address> = env.storage().instance().get(&DataKey::Admin);
        if existing.is_some() {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Admin)
    }

    // ---------- Participants ----------

    pub fn register_participant(
        env: Env,
        address: Address,
        role: Role,
        name: String,
        metadata: String,
    ) -> Result<(), Error> {
        address.require_auth();

        if Participant::load(&env, &DataKey::Participant(address.clone())).is_some() {
            return Err(Error::AlreadyRegistered);
        }

        let participant = Participant {
            address: address.clone(),
            role,
            name,
            metadata,
            registered_at: env.ledger().timestamp(),
        };
        participant.save(&env, DataKey::Participant(address));
        log!(&env, "Participant registered: {}", participant.address);
        Ok(())
    }

    pub fn get_participant(env: Env, address: Address) -> Option<Participant> {
        Participant::load(&env, &DataKey::Participant(address))
    }

    // ---------- Products ----------

    pub fn register_product(
        env: Env,
        caller: Address,
        name: String,
        origin: String,
        metadata: String,
    ) -> Result<u64, Error> {
        caller.require_auth();

        let participant =
            Participant::load(&env, &DataKey::Participant(caller.clone())).ok_or(Error::NotFound)?;

        if participant.role != Role::Farmer && participant.role != Role::Other {
            return Err(Error::WrongRole);
        }

        let mut count: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::ProductCount)
            .unwrap_or(0);
        count += 1;

        let now = env.ledger().timestamp();
        let product = Product {
            id: count,
            name,
            origin,
            owner: caller,
            status: ProductStatus::Created,
            metadata,
            created_at: now,
            updated_at: now,
        };
        product.save(&env, DataKey::Product(count));

        let mut list_len: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::ProductListLen)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::ProductList(list_len), &count);
        list_len += 1;
        env.storage()
            .persistent()
            .set(&DataKey::ProductListLen, &list_len);
        env.storage()
            .persistent()
            .set(&DataKey::ProductCount, &count);

        log!(&env, "Product registered: id={}", count);
        Ok(count)
    }

    pub fn get_product(env: Env, product_id: u64) -> Option<Product> {
        Product::load(&env, &DataKey::Product(product_id))
    }

    pub fn transfer_product(
        env: Env,
        caller: Address,
        product_id: u64,
        new_owner: Address,
    ) -> Result<(), Error> {
        caller.require_auth();

        let mut product =
            Product::load(&env, &DataKey::Product(product_id)).ok_or(Error::NotFound)?;

        if product.owner != caller {
            return Err(Error::NotAuthorized);
        }

        let _recipient =
            Participant::load(&env, &DataKey::Participant(new_owner.clone())).ok_or(Error::NotFound)?;

        product.owner = new_owner.clone();
        product.updated_at = env.ledger().timestamp();
        product.save(&env, DataKey::Product(product_id));

        let _ = Self::record_event_internal(
            &env,
            product_id,
            String::from_str(&env, "ownership_transfer"),
            String::from_str(&env, ""),
            caller,
            String::from_str(&env, ""),
        );

        log!(
            &env,
            "Product {} transferred to {}",
            product_id,
            new_owner
        );
        Ok(())
    }

    pub fn set_product_status(
        env: Env,
        caller: Address,
        product_id: u64,
        status: ProductStatus,
    ) -> Result<(), Error> {
        caller.require_auth();

        let mut product =
            Product::load(&env, &DataKey::Product(product_id)).ok_or(Error::NotFound)?;

        if product.owner != caller {
            return Err(Error::NotAuthorized);
        }

        product.status = status;
        product.updated_at = env.ledger().timestamp();
        product.save(&env, DataKey::Product(product_id));

        log!(&env, "Product {} status updated", product_id);
        Ok(())
    }

    // ---------- Events ----------

    pub fn record_event(
        env: Env,
        caller: Address,
        product_id: u64,
        event_type: String,
        location: String,
        metadata: String,
    ) -> Result<u64, Error> {
        caller.require_auth();

        let product = Product::load(&env, &DataKey::Product(product_id)).ok_or(Error::NotFound)?;

        if product.owner != caller {
            if Participant::load(&env, &DataKey::Participant(caller.clone()))
                .ok_or(Error::NotFound)?
                .role
                != Role::Inspector
            {
                return Err(Error::NotAuthorized);
            }
        }

        Self::record_event_internal(&env, product_id, event_type, location, caller, metadata)
    }

    fn record_event_internal(
        env: &Env,
        product_id: u64,
        event_type: String,
        location: String,
        actor: Address,
        metadata: String,
    ) -> Result<u64, Error> {
        let mut count: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::EventCount(product_id))
            .unwrap_or(0);
        count += 1;

        let now = env.ledger().timestamp();
        let event = Event {
            id: count,
            product_id,
            event_type,
            location,
            actor: actor.clone(),
            metadata,
            timestamp: now,
        };
        event.save(env, DataKey::Event(product_id, count));

        let mut event_count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::ProductEvents(product_id, 0))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::ProductEvents(product_id, event_count + 1), &count);
        event_count += 1;
        env.storage()
            .persistent()
            .set(&DataKey::ProductEvents(product_id, 0), &event_count);
        env.storage()
            .persistent()
            .set(&DataKey::EventCount(product_id), &count);

        log!(&env, "Event recorded: product={}, event={}", product_id, count);
        Ok(count)
    }

    pub fn get_event(env: Env, product_id: u64, event_id: u64) -> Option<Event> {
        Event::load(&env, &DataKey::Event(product_id, event_id))
    }

    // ---------- Queries ----------

    pub fn get_product_events(
        env: Env,
        product_id: u64,
        page: u32,
        page_size: u32,
    ) -> Vec<Event> {
        let total: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::ProductEvents(product_id, 0))
            .unwrap_or(0);

        if total == 0 || page_size == 0 {
            return Vec::new(&env);
        }

        let start = page * page_size;
        if start >= total {
            return Vec::new(&env);
        }

        let end = (start + page_size).min(total);
        let mut events = Vec::new(&env);

        for i in start..end {
            let event_id: u64 = env
                .storage()
                .persistent()
                .get(&DataKey::ProductEvents(product_id, i + 1))
                .unwrap_or(0);
            if let Some(event) = Event::load(&env, &DataKey::Event(product_id, event_id)) {
                events.push_back(event);
            }
        }
        events
    }

    pub fn get_product_events_count(env: Env, product_id: u64) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::ProductEvents(product_id, 0))
            .unwrap_or(0)
    }

    pub fn get_all_products(env: Env, page: u32, page_size: u32) -> Vec<Product> {
        let total: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::ProductListLen)
            .unwrap_or(0);

        if total == 0 || page_size == 0 {
            return Vec::new(&env);
        }

        let start = page * page_size;
        if start >= total {
            return Vec::new(&env);
        }

        let end = (start + page_size).min(total);
        let mut products = Vec::new(&env);

        for i in start..end {
            let product_id: u64 = env
                .storage()
                .persistent()
                .get(&DataKey::ProductList(i))
                .unwrap_or(0);
            if let Some(product) = Product::load(&env, &DataKey::Product(product_id)) {
                products.push_back(product);
            }
        }
        products
    }

    pub fn get_total_products(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::ProductListLen)
            .unwrap_or(0)
    }

    pub fn get_products_by_owner(env: Env, owner: Address) -> Vec<Product> {
        let total: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::ProductListLen)
            .unwrap_or(0);

        let mut products = Vec::new(&env);
        for i in 0..total {
            let product_id: u64 = env
                .storage()
                .persistent()
                .get(&DataKey::ProductList(i))
                .unwrap_or(0);
            if let Some(product) = Product::load(&env, &DataKey::Product(product_id)) {
                if product.owner == owner {
                    products.push_back(product);
                }
            }
        }
        products
    }
}

// ========================
// Tests
// ========================

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup() -> (Env, ProvenanceClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(Provenance, ());
        let client = ProvenanceClient::new(&env, &contract_id);
        client.init(&admin);
        (env, client)
    }

    fn register_farmer(client: &ProvenanceClient, address: &Address, name: &str) {
        client.register_participant(
            address,
            &Role::Farmer,
            &String::from_str(&client.env, name),
            &String::from_str(&client.env, "{}"),
        );
    }

    fn register_processor(client: &ProvenanceClient, address: &Address, name: &str) {
        client.register_participant(
            address,
            &Role::Processor,
            &String::from_str(&client.env, name),
            &String::from_str(&client.env, "{}"),
        );
    }

    fn register_retailer(client: &ProvenanceClient, address: &Address, name: &str) {
        client.register_participant(
            address,
            &Role::Retailer,
            &String::from_str(&client.env, name),
            &String::from_str(&client.env, "{}"),
        );
    }

    #[test]
    fn test_init() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(Provenance, ());
        let client = ProvenanceClient::new(&env, &contract_id);
        client.init(&admin);
        assert_eq!(client.admin(), Some(admin.clone()));
    }

    #[test]
    fn test_register_participant() {
        let (_env, client) = setup();
        let farmer = Address::generate(&_env);

        client.register_participant(
            &farmer,
            &Role::Farmer,
            &String::from_str(&_env, "Alice's Farm"),
            &String::from_str(&_env, "{\"location\": \"California\"}"),
        );

        let participant = client.get_participant(&farmer);
        assert!(participant.is_some());
        assert_eq!(
            participant.unwrap().name,
            String::from_str(&_env, "Alice's Farm")
        );

        let duplicate = client.try_register_participant(
            &farmer,
            &Role::Farmer,
            &String::from_str(&_env, "Duplicate"),
            &String::from_str(&_env, ""),
        );
        assert_eq!(duplicate, Err(Ok(Error::AlreadyRegistered)));
    }

    #[test]
    fn test_register_product() {
        let (_env, client) = setup();
        let farmer = Address::generate(&_env);
        register_farmer(&client, &farmer, "Green Valley Farm");

        let product_id: u64 = client.register_product(
            &farmer,
            &String::from_str(&_env, "Organic Coffee Beans"),
            &String::from_str(&_env, "Colombia, Andes Region"),
            &String::from_str(&_env, "{\"batch\": \"B-2026-001\"}"),
        );
        assert_eq!(product_id, 1);

        let product = client.get_product(&1);
        assert!(product.is_some());
        let p = product.unwrap();
        assert_eq!(p.name, String::from_str(&_env, "Organic Coffee Beans"));
        assert_eq!(p.owner, farmer);
        assert_eq!(p.status, ProductStatus::Created);
    }

    #[test]
    fn test_register_product_unregistered_participant() {
        let (_env, client) = setup();
        let stranger = Address::generate(&_env);

        let result = client.try_register_product(
            &stranger,
            &String::from_str(&_env, "Illegal Product"),
            &String::from_str(&_env, "Nowhere"),
            &String::from_str(&_env, ""),
        );
        assert_eq!(result, Err(Ok(Error::NotFound)));
    }

    #[test]
    fn test_register_product_wrong_role() {
        let (_env, client) = setup();
        let retailer = Address::generate(&_env);
        register_retailer(&client, &retailer, "Mega Store");

        let result = client.try_register_product(
            &retailer,
            &String::from_str(&_env, "Something"),
            &String::from_str(&_env, "USA"),
            &String::from_str(&_env, ""),
        );
        assert_eq!(result, Err(Ok(Error::WrongRole)));
    }

    #[test]
    fn test_record_event() {
        let (_env, client) = setup();
        let farmer = Address::generate(&_env);
        register_farmer(&client, &farmer, "Sunrise Orchard");

        let product_id: u64 = client.register_product(
            &farmer,
            &String::from_str(&_env, "Organic Apples"),
            &String::from_str(&_env, "Washington"),
            &String::from_str(&_env, ""),
        );

        let event_id: u64 = client.record_event(
            &farmer,
            &product_id,
            &String::from_str(&_env, "harvest"),
            &String::from_str(&_env, "Orchard Block A"),
            &String::from_str(&_env, "{\"weight_kg\": 500}"),
        );
        assert_eq!(event_id, 1);

        let event = client.get_event(&product_id, &1);
        assert!(event.is_some());
        assert_eq!(
            event.unwrap().event_type,
            String::from_str(&_env, "harvest")
        );
    }

    #[test]
    fn test_event_pagination() {
        let (_env, client) = setup();
        let farmer = Address::generate(&_env);
        register_farmer(&client, &farmer, "Big Farm");

        let product_id: u64 = client.register_product(
            &farmer,
            &String::from_str(&_env, "Wheat"),
            &String::from_str(&_env, "Kansas"),
            &String::from_str(&_env, ""),
        );

        for _i in 0..10 {
            client.record_event(
                &farmer,
                &product_id,
                &String::from_str(&_env, "status_check"),
                &String::from_str(&_env, "Warehouse"),
                &String::from_str(&_env, "{}"),
            );
        }

        let count = client.get_product_events_count(&product_id);
        assert_eq!(count, 10);

        let page1 = client.get_product_events(&product_id, &0, &3);
        assert_eq!(page1.len(), 3);

        let page4 = client.get_product_events(&product_id, &3, &3);
        assert_eq!(page4.len(), 1);

        let past_end = client.get_product_events(&product_id, &10, &3);
        assert_eq!(past_end.len(), 0);
    }

    #[test]
    fn test_transfer_product() {
        let (_env, client) = setup();
        let farmer = Address::generate(&_env);
        let processor = Address::generate(&_env);
        register_farmer(&client, &farmer, "Green Acres");
        register_processor(&client, &processor, "Mighty Mill");

        let product_id: u64 = client.register_product(
            &farmer,
            &String::from_str(&_env, "Corn"),
            &String::from_str(&_env, "Iowa"),
            &String::from_str(&_env, ""),
        );

        client.transfer_product(&farmer, &product_id, &processor);

        let product = client.get_product(&product_id).unwrap();
        assert_eq!(product.owner, processor);

        let unauthorized = Address::generate(&_env);
        let result = client.try_transfer_product(&unauthorized, &product_id, &farmer);
        assert_eq!(result, Err(Ok(Error::NotAuthorized)));
    }

    #[test]
    fn test_set_product_status() {
        let (_env, client) = setup();
        let farmer = Address::generate(&_env);
        register_farmer(&client, &farmer, "Happy Farm");

        let product_id: u64 = client.register_product(
            &farmer,
            &String::from_str(&_env, "Tomatoes"),
            &String::from_str(&_env, "Florida"),
            &String::from_str(&_env, ""),
        );

        client.set_product_status(&farmer, &product_id, &ProductStatus::InTransit);

        let product = client.get_product(&product_id).unwrap();
        assert_eq!(product.status, ProductStatus::InTransit);
    }

    #[test]
    fn test_get_all_products() {
        let (_env, client) = setup();
        let farmer = Address::generate(&_env);
        register_farmer(&client, &farmer, "Variety Farm");

        for _i in 0..5 {
            client.register_product(
                &farmer,
                &String::from_str(&_env, "Product"),
                &String::from_str(&_env, "USA"),
                &String::from_str(&_env, ""),
            );
        }

        let total = client.get_total_products();
        assert_eq!(total, 5);

        let all = client.get_all_products(&0, &10);
        assert_eq!(all.len(), 5);

        let page = client.get_all_products(&1, &2);
        assert_eq!(page.len(), 2);
    }

    #[test]
    fn test_get_products_by_owner() {
        let (_env, client) = setup();
        let farmer1 = Address::generate(&_env);
        let farmer2 = Address::generate(&_env);
        register_farmer(&client, &farmer1, "Farm One");
        register_farmer(&client, &farmer2, "Farm Two");

        client.register_product(
            &farmer1,
            &String::from_str(&_env, "A"),
            &String::from_str(&_env, ""),
            &String::from_str(&_env, ""),
        );

        client.register_product(
            &farmer2,
            &String::from_str(&_env, "B"),
            &String::from_str(&_env, ""),
            &String::from_str(&_env, ""),
        );

        client.register_product(
            &farmer1,
            &String::from_str(&_env, "C"),
            &String::from_str(&_env, ""),
            &String::from_str(&_env, ""),
        );

        let f1_products = client.get_products_by_owner(&farmer1);
        assert_eq!(f1_products.len(), 2);

        let f2_products = client.get_products_by_owner(&farmer2);
        assert_eq!(f2_products.len(), 1);
    }

    #[test]
    fn test_full_supply_chain_scenario() {
        let (_env, client) = setup();
        let farmer = Address::generate(&_env);
        let processor = Address::generate(&_env);
        let shipper = Address::generate(&_env);
        let retailer = Address::generate(&_env);
        let inspector = Address::generate(&_env);

        register_farmer(&client, &farmer, "Coffee Plantation Co.");
        register_processor(&client, &processor, "Artisan Roasters");
        register_retailer(&client, &retailer, "Downtown Cafe");

        client.register_participant(
            &shipper,
            &Role::Shipper,
            &String::from_str(&_env, "Global Logistics Inc."),
            &String::from_str(&_env, ""),
        );

        client.register_participant(
            &inspector,
            &Role::Inspector,
            &String::from_str(&_env, "Quality Certifiers Ltd."),
            &String::from_str(&_env, ""),
        );

        let product_id: u64 = client.register_product(
            &farmer,
            &String::from_str(&_env, "Single Origin Coffee"),
            &String::from_str(&_env, "Ethiopia, Yirgacheffe Region"),
            &String::from_str(&_env, "{\"altitude\": \"1800m\", \"variety\": \"Heirloom\"}"),
        );

        client.record_event(
            &farmer,
            &product_id,
            &String::from_str(&_env, "harvest"),
            &String::from_str(&_env, "Block B, 1800m"),
            &String::from_str(&_env, "{\"weight_kg\": 1000}"),
        );

        client.record_event(
            &inspector,
            &product_id,
            &String::from_str(&_env, "inspection"),
            &String::from_str(&_env, "Farm Warehouse"),
            &String::from_str(&_env, "{\"grade\": \"A\", \"passed\": true}"),
        );

        client.transfer_product(&farmer, &product_id, &processor);

        client.record_event(
            &processor,
            &product_id,
            &String::from_str(&_env, "roasting"),
            &String::from_str(&_env, "Roasting Facility #3"),
            &String::from_str(&_env, "{\"batch_size_kg\": 500}"),
        );

        client.transfer_product(&processor, &product_id, &shipper);

        client.record_event(
            &shipper,
            &product_id,
            &String::from_str(&_env, "shipping"),
            &String::from_str(&_env, "Port of Djibouti"),
            &String::from_str(&_env, "{\"container\": \"MSCU1234567\"}"),
        );

        client.transfer_product(&shipper, &product_id, &retailer);

        client.set_product_status(&retailer, &product_id, &ProductStatus::Delivered);

        let product = client.get_product(&product_id).unwrap();
        assert_eq!(product.status, ProductStatus::Delivered);
        assert_eq!(product.owner, retailer);

        let events = client.get_product_events(&product_id, &0, &10);
        assert_eq!(events.len(), 7);
    }
}
