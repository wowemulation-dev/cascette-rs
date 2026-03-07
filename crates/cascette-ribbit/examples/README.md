# cascette-ribbit Examples

## simple_server

Starts a Ribbit server with a small in-memory test database containing sample WoW
builds. Listens on HTTP (port 8080) and TCP (port 1119). Supports v1 (MIME-wrapped)
and v2 protocol queries.

```sh
cargo run -p cascette-ribbit --example simple_server
```

Test with:
```sh
# HTTP
curl http://localhost:8080/wow/versions
curl http://localhost:8080/wow/cdns

# TCP v2
echo "v2/products/wow/versions" | nc localhost 1119

# TCP v1
echo "v1/products/wow/versions" | nc localhost 1119
echo "v1/summary" | nc localhost 1119
```

**Prerequisites:** Ports 8080 and 1119 must be available.
