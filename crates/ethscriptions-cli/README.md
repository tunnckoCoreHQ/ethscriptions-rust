# eths — Ethscriptions CLI

Command-line tool for interacting with the Ethscriptions protocol. Built on top
of the `ethscriptions-sdk` Rust library crate.

## Install

```sh
cargo install --path crates/ethscriptions-cli
```

This installs the binary as `eths`.

## Usage

```
eths [OPTIONS] <COMMAND>
```

### Global options

| Flag               | Description                                                   |
| ------------------ | ------------------------------------------------------------- |
| `--pretty`         | Output JSON in pretty-printed format                          |
| `--base-url <URL>` | Override the upstream API base URL (also `ETHS_BASE_URL` env) |

### Commands

| Command      | Description                                                       |
| ------------ | ----------------------------------------------------------------- |
| `get`        | Fetch a single ethscription by transaction hash or number         |
| `list`       | List ethscriptions with optional filters                          |
| `exists`     | Check if content exists as an ethscription (by SHA or file/stdin) |
| `content`    | Download ethscription content                                     |
| `attachment` | Download ethscription attachment                                  |
| `resolve`    | Resolve an ENS name, ethscription name, or address                |
| `sha`        | Compute SHA-256 digest of data                                    |
| `gas`        | Fetch current gas prices                                          |
| `estimate`   | Estimate ethscription creation cost                               |
| `meta`       | Detect content type and metadata of input data                    |
| `validate`   | Validate addresses, hashes, hex strings, and more                 |

## Examples

### Compute SHA-256

```sh
eths sha "hello"
# 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824

eths sha --json "hello"
# {"sha":"2cf24dba...","hex":"68656c6c6f","data_uri":"hello"}

eths sha --name wgw | eths exists --pretty
{
  "exists": true,
  "ethscription": {
    "ethscription_id": "0xac9cbabc7425e9ea665be6dc6479b1dd22ef14ef586698af3c650447c07f6214",
    "block_number": "17701609",
    "block_blockhash": "0x30ba277e7a0cff9248883679054cf4156712ab6da1601bc5492b50b8ba22023f",
    "block_timestamp": "1689458687",
    "block_datetime": "2023-07-15T22:04:47.000Z",
    "transaction_hash": "0xac9cbabc7425e9ea665be6dc6479b1dd22ef14ef586698af3c650447c07f6214",
    "transaction_index": "93",
    "transaction_value": "0",
    "transaction_fee": "244966778186040",
    "gas_price": "11585640285",
    "gas_used": "21144",
    "creator": "0x2052051a0474fb0b98283b3f38c13b0b0b6a3677",
    "receiver": "0x2052051a0474fb0b98283b3f38c13b0b0b6a3677",
    "media_type": "text",
    "media_subtype": "plain",
    "content_type": "text/plain",
    "content_sha": "0x161cfbb8a29429c151f2b57a9c5f9a35dab57a63aacedd5803472c5bda8ec5f9",
    "content_path": "/ethscriptions/0xac9cbabc7425e9ea665be6dc6479b1dd22ef14ef586698af3c650447c07f6214/content",
    "is_esip0": true,
    "is_esip3": false,
    "is_esip4": false,
    "is_esip6": false,
    "is_esip8": false
  }
}

eths sha --file ./package.json | eths exists
{"exists":false}

# both `sha` and `exists` commands support stdin input
cat ./package.json | eths sha | eths exists
{"exists":false}
```

### Validate values

```sh
eths validate address 0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045
# true

eths validate hash 0x2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
# true

eths validate --json address 0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045
# {"valid":true,"value":"0xd8dA...","kind":"address"}
```

Available sub-commands: `address`, `hash`, `hex`, `sha256`, `data-uri`, `id`.

### Detect content type

```sh
echo '{"name":"test"}' | eths meta
# {"content_type":"application/json","size":16,"sha256":"d255..."}

echo '<svg xmlns="http://www.w3.org/2000/svg"></svg>' | eths meta
# {"content_type":"image/svg+xml","size":47,"sha256":"7b3b..."}
```

### Fetch an ethscription

```sh
eths get 123
eths get 0x1234abcd...
```

### Get content of ethscription

```sh
eths content 123 --output punk.png

# output to stdout
eths content 0x1234abcd...
```

### Pipe-friendly

Most commands accept input from stdin, making them composable in shell
pipelines:

```sh
# Pipe a SHA into exists check
eths sha "hello" | eths exists

# Pipe an address into validate
echo "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045" | eths validate address

# Pipe an ID into get
echo 12345 | eths get

# Pipe a SHA into validate
eths sha "hello" | eths validate sha256
```

### Pretty print

Add `--pretty` before the command for indented JSON:

```sh
eths --pretty sha --json "hello"
# {
#   "sha": "2cf24dba...",
#   "hex": "68656c6c6f",
#   "data_uri": "hello"
# }
```
