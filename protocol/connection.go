package protocol

import (
	"github.com/rs/zerolog/log"
	"github.com/skyezerfox/moss/constants"
	"github.com/Tnze/go-mc/data"
	"github.com/Tnze/go-mc/net"
	"github.com/Tnze/go-mc/net/packet"
	"math/rand"
	"time"
)

// Connection Represents a proxied player and their connection.
type Connection struct {
	conn          *net.Conn
	state         int
	username      string
	uuid		  string
	verifyToken   []byte
	entityID      int32
	settings      ClientSettings
	stopKeepalive chan struct{}
	lastKeepalive time.Time
	cm            *ConnectionManager
}

type ClientSettings struct {
	Locale             string
	ViewDistance       byte
	ChatMode           int
	ChatColors         bool
	DisplayedSkinParts byte
	MainHand           int
}

func (c *Connection) HandlePlayer() {
	for {
		input, err := c.conn.ReadPacket()
		if err != nil {
			break
		}
		log.Debug().Str("addr", c.conn.Socket.RemoteAddr().String()).Int32("id", input.ID).Int("state", c.state).Msg("PACKET")

		switch c.state {
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
	if c.state == constants.Play {
		c.cm.RemovePlayer(c)
	}
	log.Info().Msgf("%s disconnected", c.conn.Socket.RemoteAddr().String())
	if c.stopKeepalive != nil {
		close(c.stopKeepalive)
	}
}

func (c *Connection) sendDisconnect(message string) {
	c.conn.WritePacket(packet.Marshal(
		0x0,
		packet.Chat(message),
	))
	c.conn.Close()
}

func (c *Connection) keepalive() {
	for {
		if time.Now().Sub(c.lastKeepalive) > time.Second*20 {
			log.Warn().Msgf("%s timed out", c.username)
			c.conn.Close()
			return
		}
		select {
		case <-c.stopKeepalive:
			log.Debug().Str("username", c.username).Msg("Stopping keepalive")
			return
		default:
			c.conn.WritePacket(
				packet.Marshal(
					data.KeepAliveClientbound,
					packet.Long(rand.Int()),
				),
			)
		}
		time.Sleep(5 * time.Second)
	}
}


func (c *Connection) changeState(state int) {
	if state == constants.Play {
		// TODO set up keepalive
		c.lastKeepalive = time.Now()
		c.stopKeepalive = make(chan struct{})
		go c.keepalive()
	}
	c.state = state
}
