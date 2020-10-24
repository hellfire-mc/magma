package protocol

import (
	"github.com/Tnze/go-mc/net/packet"
	"github.com/rs/zerolog/log"
)

func (c *Connection) handleHandshakeState(input *packet.Packet) {
	switch input.ID {
	case 0x00:
		c.handleHandshake(input)
	}
}

func (c *Connection) handleHandshake(input *packet.Packet) {
	var (
		version packet.VarInt
		address packet.String
		port    packet.UnsignedShort
		state   packet.VarInt
	)

	err := input.Scan(&version, &address, &port, &state)
	if err != nil {
		return
	}

	log.Info().Str("addr", c.conn.Socket.RemoteAddr().String()).Int("version", int(version)).Int("state", int(state)).Msg("Handshake successful")
	c.state = int(state)
}
