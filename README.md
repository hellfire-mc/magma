# magma

Magma is a domain-switching reverse proxy for Minecraft servers.

## Configuration

### Example Configuration

- `debug`: boolean - enable debug logging
- `online`: boolean - whether to use [online mode](https://minecraft.fandom.com/wiki/Server.properties#Java_Edition_3)

- `listener`: map - 

```toml
# Config file version - do not edit!
version = 1

# Enable debug logging
debug = false
# Enable online mode
online = true

[listener]
# The bind address of the server
bind_address = "0.0.0.0"
# The listening port of the server
port = 25565

# A server proxied by Moss.
[[servers]]
# The domain of the server.
domain = "mc.skzr.dev"
# The forwarding target.
target = "172.18.0.1:34001"

[[servers]]
# A list of domains supported by this server entry.
domains = [
	"mc.kaylen.dog",
	"play.kaylen.dog"
]
selection_algorithm = "random" # One of "random", "round-robin"
# A list of targets supported by this server entry.
targets = [
	"172.18.0.1:34001",
	"172.18.0.1:34002"
]

```

## License

Magma is licensed under the GNU Affero General Public License version 3.0.
