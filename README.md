# ZachDB

A lightweight, high-performance, non-relational database built in Rust.

## Features

- Incredibly lightweight and fast
- Non-relational document store
- Automatic indexing out of the box
- Highly concurrent - handles thousands of simultaneous reads and writes
- REST API for easy application integration
- Persistent storage with efficient serialization

## Getting Started

### Prerequisites

- Rust 1.65 or later

### Installation

```bash
# Clone the repository
git clone https://github.com/zachwilke/zachdb.git
cd zachdb

# Build the project
cargo build --release

# Run the database
./target/release/zachdb
```

### Usage

Once running, ZachDB will be available on `http://localhost:7878` by default. 
You can configure the port and other settings via command-line options or a config file.

## API Reference

### Collections

- `GET /collections` - List all collections
- `POST /collections` - Create a new collection
- `DELETE /collections/{name}` - Delete a collection

### Documents

- `GET /collections/{name}/documents` - List all documents in a collection
- `GET /collections/{name}/documents/{id}` - Get a specific document
- `POST /collections/{name}/documents` - Create a new document
- `PUT /collections/{name}/documents/{id}` - Update a document
- `DELETE /collections/{name}/documents/{id}` - Delete a document

### Queries

- `POST /collections/{name}/query` - Query documents in a collection

## License

MIT

## Author

Zach Wilke 
