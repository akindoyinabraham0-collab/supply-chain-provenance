# Supply Chain Provenance on Stellar/Soroban

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.96%2B-blue)](rust-toolchain.toml)
[![Soroban](https://img.shields.io/badge/Soroban-SDK%2027-red)](contracts/provenance/Cargo.toml)
[![Tests](https://img.shields.io/badge/Tests-12%2F12-passing-brightgreen)](contracts/provenance/src/lib.rs)

A decentralized supply chain provenance tracker built on Stellar's Soroban smart contract platform. Enables transparent, tamper-proof tracking of products from origin to consumer, with immutable audit trails and role-based access control.

## Overview

Supply chain transparency is one of the most pressing challenges in global trade. This smart contract provides a trustless mechanism for:

- **Registering** participants with verifiable roles (farmer, processor, shipper, retailer, inspector)
- **Registering** products with cryptographic proof of origin and metadata
- **Recording** every event in a product's lifecycle with timestamps and location data
- **Transferring** custody between participants with automatic audit trail entries
- **Querying** the complete history of any product via paginated event logs

## The Problem

- **Counterfeit goods**: $4.5 trillion lost annually to counterfeit products
- **Supply chain fraud**: $40+ billion lost annually across industries
- **Counterfeit medications**: 250,000+ deaths per year from fake pharmaceuticals
- **Consumer trust**: 73% of consumers don't trust sustainability claims
- **Food safety**: Traceability failures cause massive recalls and health risks
- **Ethical sourcing**: No verifiable mechanism to prove fair trade / organic claims

## Features

- **Role-Based Access Control** — Six distinct participant roles with granular permissions
- **Immutable Event Log** — Every action creates an on-chain event with timestamp and actor
- **Automatic Audit Trail** — Ownership transfers automatically record events
- **Pagination** — Efficiently query products and events with page/page_size
- **Metadata** — Arbitrary JSON metadata for participants, products, and events
- **Inspector Access** — Inspectors can record events on any product without ownership
- **Comprehensive Error Handling** — Well-defined error codes for all failure modes

## Architecture

```
supply-chain-provenance/
├── Cargo.toml                  # Workspace config
├── rust-toolchain.toml         # Pinned Rust toolchain
├── .gitignore
├── .gitattributes
├── LICENSE                     # MIT
├── README.md
├── build.ps1                   # PowerShell build script
├── contracts/
│   └── provenance/
│       ├── Cargo.toml          # Contract dependencies
│       └── src/
│           └── lib.rs          # Full contract logic + unit tests
└── tests/                      # Integration tests (optional)
```

### Storage Architecture

The contract uses Soroban persistent storage with these storage key categories:

| Key Pattern | Type | Purpose |
|---|---|---|
| `Admin` | Instance | Contract admin address (single value) |
| `Participant(address)` | Persistent | Participant data per address |
| `Product(id)` | Persistent | Product data per ID |
| `ProductCount` | Persistent | Auto-incrementing product ID counter |
| `ProductListLen` | Persistent | Total number of registered products |
| `ProductList(index)` | Persistent | Ordered list of product IDs for pagination |
| `Event(product_id, event_id)` | Persistent | Individual event data |
| `EventCount(product_id)` | Persistent | Auto-incrementing event ID per product |
| `ProductEvents(product_id, 0)` | Persistent | Event list length for product (index 0 reserved) |
| `ProductEvents(product_id, i)` | Persistent | Ordered list of event IDs for product pagination |

> **Note:** `ProductEvents(product_id, 0)` stores the event count. Actual event IDs are stored at `ProductEvents(product_id, 1)` through `ProductEvents(product_id, count)`. This avoids the prior bug where index 0 was used both as counter and event storage.

## Data Model

### Roles

| Role | Can Register Products | Can Record Events | Can Transfer | Default Permission |
|---|---|---|---|---|
| `Farmer` | Yes | Yes (own products) | Yes (own) | Product originator |
| `Processor` | No | Yes (own products) | Yes (own) | Manufacturing |
| `Shipper` | No | Yes (own products) | Yes (own) | Logistics |
| `Retailer` | No | Yes (own products) | Yes (own) | End seller |
| `Inspector` | No | Yes (any product) | No | Audit/quality check |
| `Other` | Yes | Yes (own products) | Yes (own) | Custom use |

### Product Status Lifecycle

```
Created → InTransit → Processed → Inspected → Delivered
                ↘           ↘          ↘
                Recalled    Recalled   Recalled
                                     Archived
```

### Participant

| Field | Type | Description |
|---|---|---|
| `address` | `Address` | Stellar account address |
| `role` | `Role` | One of: `Farmer`, `Processor`, `Shipper`, `Retailer`, `Inspector`, `Other` |
| `name` | `String` | Human-readable name |
| `metadata` | `String` | Arbitrary JSON metadata |
| `registered_at` | `u64` | Ledger timestamp of registration |

### Product

| Field | Type | Description |
|---|---|---|
| `id` | `u64` | Auto-generated unique product ID |
| `name` | `String` | Product name |
| `origin` | `String` | Geographic or supply origin |
| `owner` | `Address` | Current custodian |
| `status` | `ProductStatus` | Current lifecycle status |
| `metadata` | `String` | Arbitrary JSON metadata |
| `created_at` | `u64` | Ledger timestamp of creation |
| `updated_at` | `u64` | Ledger timestamp of last update |

### Event

| Field | Type | Description |
|---|---|---|
| `id` | `u64` | Auto-generated event ID (scoped per product) |
| `product_id` | `u64` | Associated product |
| `event_type` | `String` | e.g. `"harvest"`, `"inspection"`, `"shipping"`, `"ownership_transfer"` |
| `location` | `String` | Where the event occurred |
| `actor` | `Address` | Who recorded the event |
| `metadata` | `String` | Arbitrary JSON metadata |
| `timestamp` | `u64` | Ledger timestamp |

## Contract API

### Admin Functions

#### `init(admin: Address)`

Initialize the contract with an admin address. Can only be called once.

- **Auth**: `admin.require_auth()`
- **Panics**: If contract is already initialized

#### `admin() -> Option<Address>`

Returns the contract admin address, or `None` if not initialized.

### Participant Functions

#### `register_participant(address: Address, role: Role, name: String, metadata: String) -> Result<(), Error>`

Register a new supply chain participant.

- **Auth**: `address.require_auth()` (self-registration)
- **Errors**:
  - `AlreadyRegistered (1)` — Participant already exists at this address
- **Example**:
  ```bash
  stellar contract invoke --id <contract-id> --source-account <addr> --network testnet -- \
    register_participant \
    --address <addr> \
    --role '{"farmer": {}}' \
    --name "Green Valley Farm" \
    --metadata '{"location": "California"}'
  ```

#### `get_participant(address: Address) -> Option<Participant>`

Get participant details by address, or `None` if not registered.

### Product Functions

#### `register_product(caller: Address, name: String, origin: String, metadata: String) -> Result<u64, Error>`

Register a new product. Only `Farmer` or `Other` roles can register products.

- **Auth**: `caller.require_auth()`
- **Returns**: Auto-generated product ID (starts at 1, increments)
- **Errors**:
  - `NotFound (3)` — Caller is not a registered participant
  - `WrongRole (6)` — Caller's role is not `Farmer` or `Other`

#### `get_product(product_id: u64) -> Option<Product>`

Get product details by ID, or `None` if not found.

#### `transfer_product(caller: Address, product_id: u64, new_owner: Address) -> Result<(), Error>`

Transfer product custody to another registered participant. Automatically records an `"ownership_transfer"` event.

- **Auth**: `caller.require_auth()`
- **Errors**:
  - `NotFound (3)` — Product or new owner not found
  - `NotAuthorized (2)` — Caller is not the current owner

#### `set_product_status(caller: Address, product_id: u64, status: ProductStatus) -> Result<(), Error>`

Update the product's lifecycle status.

- **Auth**: `caller.require_auth()`
- **Errors**:
  - `NotFound (3)` — Product not found
  - `NotAuthorized (2)` — Caller is not the current owner

#### `get_all_products(page: u32, page_size: u32) -> Vec<Product>`

Get a paginated list of all products. Returns empty `Vec` if page is out of range.

- **Pagination**: 0-indexed. Page 0 returns the first `page_size` products.

#### `get_total_products() -> u32`

Returns the total number of registered products.

#### `get_products_by_owner(owner: Address) -> Vec<Product>`

Returns all products currently owned by the given address.

### Event Functions

#### `record_event(caller: Address, product_id: u64, event_type: String, location: String, metadata: String) -> Result<u64, Error>`

Record a lifecycle event for a product. The product owner can always record events. `Inspector` role can record events on any product.

- **Auth**: `caller.require_auth()`
- **Returns**: Auto-generated event ID (scoped per product)
- **Event Types** (convention): `"harvest"`, `"processing"`, `"inspection"`, `"shipping"`, `"receiving"`, `"quality_check"`, etc.
- **Errors**:
  - `NotFound (3)` — Product not found or caller not registered
  - `NotAuthorized (2)` — Caller is neither owner nor `Inspector`

#### `get_event(product_id: u64, event_id: u64) -> Option<Event>`

Get a specific event by product and event ID, or `None`.

#### `get_product_events(product_id: u64, page: u32, page_size: u32) -> Vec<Event>`

Get paginated events for a product. Events are returned in chronological order.

- **Pagination**: 0-indexed. Page 0 returns the first `page_size` events.

#### `get_product_events_count(product_id: u64) -> u32`

Returns the total number of events for a product.

### Automatic Events

The following actions automatically record events:

| Action | Event Type Recorded | Actor |
|---|---|---|
| `transfer_product` | `"ownership_transfer"` | Previous owner |

## Error Reference

| Code | Name | Description |
|---|---|---|
| `1` | `AlreadyRegistered` | Participant address is already registered |
| `2` | `NotAuthorized` | Caller lacks permission for the operation |
| `3` | `NotFound` | Product or participant not found |
| `4` | `AlreadyExists` | Resource already exists |
| `5` | `BadRequest` | Invalid input parameters |
| `6` | `WrongRole` | Caller's role does not permit the operation |

## Prerequisites

- **Rust toolchain** (pinned in `rust-toolchain.toml` — auto-installed by rustup on first build)
- **wasm32v1-none target** (auto-installed by rustup on first build via `rust-toolchain.toml`)
- **Stellar CLI** (optional, for deployment):
  ```bash
  cargo install --locked stellar-cli
  ```

## Quick Start

### Build

```bash
# Compile the contract to WASM
cargo build --target wasm32v1-none --release
```

The compiled WASM file will be at:
`target/wasm32v1-none/release/provenance.wasm`

### Optimize (optional)

```bash
stellar contract optimize --wasm target/wasm32v1-none/release/provenance.wasm
```

### Test

```bash
cargo test
```

All 12 unit tests should pass, covering:
- Contract initialization
- Participant registration (including duplicate detection)
- Product registration (including error cases: unregistered, wrong role)
- Event recording and retrieval
- Event pagination
- Product transfer (including authorization check)
- Product status updates
- Paginated product listing
- Products-by-owner queries
- Full end-to-end supply chain scenario

## Deployment

### 1. Deploy to Testnet

```bash
stellar contract deploy \
  --wasm target/wasm32v1-none/release/provenance.wasm \
  --alias provenance \
  --source-account <your-account> \
  --network testnet
```

This returns a contract ID. Save it for subsequent commands.

### 2. Initialize

```bash
stellar contract invoke \
  --id <contract-id> \
  --source-account <admin> \
  --network testnet \
  -- \
  init \
  --admin <admin-address>
```

### 3. Register Participants

```bash
# Register a farmer
stellar contract invoke \
  --id <contract-id> \
  --source-account <farmer-address> \
  --network testnet \
  -- \
  register_participant \
  --address <farmer-address> \
  --role '{"farmer": {}}' \
  --name "Green Valley Farm" \
  --metadata '{"location": "California"}'

# Register a processor
stellar contract invoke \
  --id <contract-id> \
  --source-account <processor-address> \
  --network testnet \
  -- \
  register_participant \
  --address <processor-address> \
  --role '{"processor": {}}' \
  --name "Mighty Mill Corp" \
  --metadata '{"certification": "organic"}'

# Register a retailer
stellar contract invoke \
  --id <contract-id> \
  --source-account <retailer-address> \
  --network testnet \
  -- \
  register_participant \
  --address <retailer-address> \
  --role '{"retailer": {}}' \
  --name "Downtown Grocers" \
  --metadata '{}'
```

### 4. Register a Product

```bash
stellar contract invoke \
  --id <contract-id> \
  --source-account <farmer-address> \
  --network testnet \
  -- \
  register_product \
  --caller <farmer-address> \
  --name "Organic Coffee Beans" \
  --origin "Colombia, Andes Region" \
  --metadata '{"batch": "B-2026-001", "variety": "Arabica"}'
```

### 5. Record Events

```bash
# Record a harvest event
stellar contract invoke \
  --id <contract-id> \
  --source-account <farmer-address> \
  --network testnet \
  -- \
  record_event \
  --caller <farmer-address> \
  --product_id 1 \
  --event_type "harvest" \
  --location "Block A, 1800m elevation" \
  --metadata '{"weight_kg": 1000, "method": "hand-picked"}'

# Record an inspection (inspector can record on any product)
stellar contract invoke \
  --id <contract-id> \
  --source-account <inspector-address> \
  --network testnet \
  -- \
  record_event \
  --caller <inspector-address> \
  --product_id 1 \
  --event_type "inspection" \
  --location "Farm Warehouse" \
  --metadata '{"grade": "A", "passed": true, "inspector_id": "USDA-001"}'
```

### 6. Transfer Ownership

```bash
# Farmer transfers to processor
stellar contract invoke \
  --id <contract-id> \
  --source-account <farmer-address> \
  --network testnet \
  -- \
  transfer_product \
  --caller <farmer-address> \
  --product_id 1 \
  --new_owner <processor-address>
```

This automatically records an `"ownership_transfer"` event.

### 7. Query Data

```bash
# Get product details
stellar contract invoke \
  --id <contract-id> \
  --source-account <any-address> \
  --network testnet \
  -- \
  get_product \
  --product_id 1

# Get event history (paginated, 3 per page)
stellar contract invoke \
  --id <contract-id> \
  --source-account <any-address> \
  --network testnet \
  -- \
  get_product_events \
  --product_id 1 \
  --page 0 \
  --page_size 3

# Get all products by owner
stellar contract invoke \
  --id <contract-id> \
  --source-account <any-address> \
  --network testnet \
  -- \
  get_products_by_owner \
  --owner <farmer-address>
```

## End-to-End Scenario

A complete supply chain flow (coffee from farm to cafe):

1. **Admin** initializes the contract
2. **Participants register**: Farmer, Processor, Shipper, Retailer, Inspector
3. **Farmer registers** product "Single Origin Coffee" from Ethiopia
4. **Farmer records** harvest event with weight and location
5. **Inspector records** quality inspection event (grade A, passed)
6. **Farmer transfers** custody to Processor
7. **Processor records** roasting event with batch details
8. **Processor transfers** to Shipper
9. **Shipper records** shipping event with container ID
10. **Shipper transfers** to Retailer
11. **Retailer sets** status to `Delivered`
12. **Anyone queries** the complete 7-event history for full provenance

This scenario is validated in `test_full_supply_chain_scenario`.

## Security

- **Authentication**: Every state-changing function requires `require_auth()` from the caller's Stellar address
- **Ownership Enforcement**: Only the current product owner can transfer or update status
- **Role Enforcement**: Product registration restricted to `Farmer` and `Other` roles
- **Inspector Privilege**: Inspectors can record events on any product but cannot transfer ownership
- **Single Initialization**: `init()` panics if called twice
- **Duplicate Prevention**: Participants cannot register the same address twice
- **Immutability**: Events are append-only; once recorded, they cannot be modified or deleted
- **Automatic Audit Trail**: Every ownership transfer generates a timestamped, actor-signed event

## Contributing

Contributions are welcome! This project is MIT-licensed.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Run tests (`cargo test`) and WASM build (`cargo build --target wasm32v1-none --release`)
5. Push to the branch (`git push origin feature/amazing-feature`)
6. Open a Pull Request

### Development Guidelines

- Follow existing code style (no `#![no_std]` incompatible dependencies)
- All new features should include unit tests
- Maintain the existing error handling patterns
- Document any new public functions in the README API section

## License

Distributed under the MIT License. See [LICENSE](LICENSE) for more information.
