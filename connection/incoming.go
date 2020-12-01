package connection

import (
	"github.com/Tnze/go-mc/net"
	"github.com/Tnze/go-mc/net/packet"
	"github.com/Tnze/go-mc/data"
	"github.com/skyezerfox/moss/constants"
	"github.com/rs/zerolog/log"
	"sync"
	"time"
	"math/rand"
	"fmt"
)

// Incoming represents the incoming connection from the player's
// Minecraft client.
type Incoming struct {
	sync.Mutex

	Player 		*Player
	CM         *ConnectionManager

	Connection *net.Conn

	VerifyToken   []byte
	StopKeepalive chan struct{}
	LastKeepalive time.Time
}

// CreateIncoming returns a new incoming connection.
func CreateIncoming(cm *ConnectionManager, p *Player) (*Incoming) {
	return &Incoming {
		Player: p,
		CM: cm,
	}
}

// HandlePlayer accepts a new packet and handles it.
func (c *Incoming) HandlePlayer() {
	for {
		input, err := c.Connection.ReadPacket()
		if err != nil {
			break
		}
		log.Debug().Str("addr", c.Connection.Socket.RemoteAddr().String()).Int32("id", input.ID).Int("state", c.Player.State).Msg("PACKET")

		switch c.Player.State {
		case constants.Handshaking:
			c.handleHandshakeState(&input)
		case constants.Status:
			c.handleStatusState(&input)
		case constants.Login:
			c.handleLoginState(&input)
			// case Play:
			// 	c.handlePlayState(&input)
		}
	}
	if c.Player.State == constants.Play {
		c.CM.RemovePlayer(c.Player)
	}
	log.Info().Msgf("%s disconnected", c.Connection.Socket.RemoteAddr().String())
	if c.StopKeepalive != nil {
		close(c.StopKeepalive)
	}
}

// SendDisconnect disconnects this player with the target reason.
func (c *Incoming) SendDisconnect(message string) {
	c.Connection.WritePacket(packet.Marshal(
		0x0,
		packet.Chat(fmt.Sprintf("{\"text\": \"%s\"}", message)),
	))
	c.Connection.Close()
}

func (c *Incoming) keepalive() {
	for {
		if time.Now().Sub(c.LastKeepalive) > time.Second*20 {
			log.Warn().Msgf("%s timed out", c.Player.Username)
			c.Connection.Close()
			return
		}
		select {
		case <-c.StopKeepalive:
			log.Debug().Str("username", c.Player.Username).Msg("Stopping keepalive")
			return
		default:
			c.Connection.WritePacket(
				packet.Marshal(
					data.KeepAliveClientbound,
					packet.Long(rand.Int()),
				),
			)
		}
		time.Sleep(5 * time.Second)
	}
}

// ChangeState changes the state of this connection.
func (c *Incoming) ChangeState(state int) {
	if state == constants.Play {
		c.LastKeepalive = time.Now()
		c.StopKeepalive = make(chan struct{})
		go c.keepalive()
	}
	c.Player.State = state
}
