# Supply Chain Provenance on Stellar/Soroban

A decentralized supply chain provenance tracker built on Stellar's Soroban smart contract platform. Enables transparent, tamper-proof tracking of products from origin to consumer.

## The Problem

- Counterfeit goods: \$4.5 trillion lost annually
- Supply chain fraud: \$40+ billion lost annually
- Counterfeit medications: 250,000+ deaths per year
- 73% of consumers don't trust sustainability claims

## Smart Contract Features

- **Participant Management** — Register supply chain actors (farmers, processors, shippers, retailers, inspectors) with roles
- **Product Registration** — Register products with origin, metadata, and cryptographic proof of authenticity
- **Event Tracking** — Record every supply chain step (harvest, processing, shipping, inspection) with timestamps and location
- **Ownership Transfer** — Transfer custody on-chain with full audit trail
- **Pagination & Queries** — Query products by owner, paginate events, list all products

## Architecture

```
supply-chain-provenance/
├── Cargo.toml              # Workspace config
├── contracts/
│   └── provenance/         # Main contract
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs      # Contract logic + tests
```

### Data Model

```
Participant { address, role, name, metadata, registered_at }
Product     { id, name, origin, owner, status, metadata, created_at, updated_at }
Event       { id, product_id, event_type, location, actor, metadata, timestamp }
```

### Roles

- `Farmer` — Can register products and record events
- `Processor` — Can receive and process products
- `Shipper` — Can handle logistics events
- `Retailer` — Can receive final products
- `Inspector` — Can record inspection events on any product
- `Other` — General purpose

### Product Statuses

- `Created` → `InTransit` → `Processed` → `Inspected` → `Delivered`
- Special: `Recalled`, `Archived`

## Prerequisites

- Rust (see `rust-toolchain.toml` for pinned version)
- wasm32 target: `rustup target add wasm32v1-none`
- Stellar CLI: `cargo install --locked stellar-cli`

## Build

```bash
# Compile the contract to WASM
cd supply-chain-provenance
cargo build --target wasm32v1-none --release
```

The compiled `.wasm` file will be at:
`target/wasm32v1-none/release/provenance.wasm`

### Optimize (optional)

```bash
stellar contract optimize --wasm target/wasm32v1-none/release/provenance.wasm
```

## Test

```bash
cargo test
```

## Deploy

### 1. Deploy to Testnet

```bash
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/provenance.wasm \
  --alias provenance \
  --source-account <your-account> \
  --network testnet
```

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

### 3. Register a participant

```bash
stellar contract invoke \
  --id <contract-id> \
  --source-account <farmer-address> \
  --network testnet \
  -- \
  register_participant \
  --address <farmer-address> \
  --role '{"farmer": {}}' \
  --name "Green Valley Farm" \
  --metadata "{\"location\": \"California\"}"
```

### 4. Register a product

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
  --metadata "{\"batch\": \"B-2026-001\"}"
```

### 5. Record an event

```bash
stellar contract invoke \
  --id <contract-id> \
  --source-account <farmer-address> \
  --network testnet \
  -- \
  record_event \
  --caller <farmer-address> \
  --product_id 1 \
  --event_type "harvest" \
  --location "Block A" \
  --metadata "{\"weight_kg\": 500}"
```

### 6. Query a product

```bash
stellar contract invoke \
  --id <contract-id> \
  --source-account <any-address> \
  --network testnet \
  -- \
  get_product \
  --product_id 1
```

## API Reference

### Admin
| Function | Parameters | Description |
|----------|-----------|-------------|
| `init` | `admin: Address` | Initialize contract with admin |
| `admin` | — | Get admin address |

### Participants
| Function | Parameters | Description |
|----------|-----------|-------------|
| `register_participant` | `address, role, name, metadata` | Register a new participant |
| `get_participant` | `address` | Get participant info |

### Products
| Function | Parameters | Description |
|----------|-----------|-------------|
| `register_product` | `caller, name, origin, metadata` | Register a new product (Farmer/Other only) |
| `get_product` | `product_id` | Get product details |
| `transfer_product` | `caller, product_id, new_owner` | Transfer custody |
| `set_product_status` | `caller, product_id, status` | Update product status |
| `get_all_products` | `page, page_size` | List all products (paginated) |
| `get_total_products` | — | Total product count |
| `get_products_by_owner` | `owner` | Get all products for an owner |

### Events
| Function | Parameters | Description |
|----------|-----------|-------------|
| `record_event` | `caller, product_id, event_type, location, metadata` | Record an event |
| `get_event` | `product_id, event_id` | Get a specific event |
| `get_product_events` | `product_id, page, page_size` | Get events for a product (paginated) |
| `get_product_events_count` | `product_id` | Total event count for a product |
