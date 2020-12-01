package connection

import (
	"github.com/Tnze/go-mc/net"
	"time"
	"github.com/spf13/viper"
	"github.com/rs/zerolog/log"
	"fmt"
)

// Outgoing represents the outgoing connection to the proxied minecraft server.
type Outgoing struct {
	Player *Player
	CM *ConnectionManager

	Connection *net.Conn

	VerifyToken   []byte
	StopKeepalive chan struct{}
	LastKeepalive time.Time
}

func handleOutgoing() {}

// CreateOutgoingConnection creates a connection to the proxied Minecraft server.
func CreateOutgoingConnection(player *Player) (*Outgoing) {

	conn, err := net.DialMC(fmt.Sprintf("%s:%d", viper.GetString("proxy.host"), viper.GetInt("proxy.port")))
	if err != nil {
		log.Err(err).Msg("Failed to open proxy connection")
		c.SendDisconnect("Failed to open proxy connection")
		return
	}

	outgoing := Outgoing {
		Connection: conn,
		Player: player,
	}

	return outgoing
}
