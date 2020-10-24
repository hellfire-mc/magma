package protocol

import (
	"encoding/json"
	"github.com/Tnze/go-mc/net/packet"
	"github.com/skyezerfox/moss/constants"
	"github.com/skyezerfox/moss/models"
	"github.com/spf13/viper"
)

func (c *Connection) handleStatusState(input *packet.Packet) {
	switch input.ID {
	case 0x00:
		c.handleStatus(input)
	case 0x01:
		c.handlePing(input)
	}
}

func (c *Connection) handleStatus(input *packet.Packet) {
	// Send server status
	out, err := json.Marshal(&models.ServerStatus{
		Version: models.Version{
			Name:     constants.MCVersion,
			Protocol: constants.MCProtocol,
		},
		Players: models.Players{
			Max:    viper.GetInt("connection.maxPlayers"),
			Online: len(c.cm.players),
			Sample: nil,
		},
		Description: models.Description{
			Text: viper.GetString("info.motd"),
		},
		// Favicon: c.cm.serverIcon,
	})
	if err != nil {
		return
	}

	c.conn.WritePacket(packet.Marshal(
		0x0,
		packet.String(string(out)),
	))
}

func (c *Connection) handlePing(input *packet.Packet) {
	var payload packet.Long
	err := input.Scan(&payload)
	if err != nil {
		return
	}

	c.conn.WritePacket(packet.Marshal(
		0x01,
		payload,
	))
}
