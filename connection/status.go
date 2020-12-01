package connection

import (
	"encoding/json"
	"github.com/Tnze/go-mc/net/packet"
	"github.com/skyezerfox/moss/constants"
	"github.com/skyezerfox/moss/models"
	"github.com/spf13/viper"
)

func (c *Incoming) handleStatusState(input *packet.Packet) {
	switch input.ID {
	case 0x00:
		c.handleStatus(input)
	case 0x01:
		c.handlePing(input)
	}
}

func (c *Incoming) handleStatus(input *packet.Packet) {
	// Send server status
	out, err := json.Marshal(&models.ServerStatus{
		Version: models.Version{
			Name:     constants.MCVersion,
			Protocol: constants.MCProtocol,
		},
		Players: models.Players{
			Max:    viper.GetInt("server.maxPlayers"),
			Online: c.CM.GetServerCount(),
			Sample: nil,
		},
		Description: models.Description{
			Text: viper.GetString("server.motd"),
		},
		// Favicon: c.cm.serverIcon,
	})
	if err != nil {
		return
	}

	c.Incoming.WritePacket(packet.Marshal(
		0x0,
		packet.String(string(out)),
	))
}

func (c *Incoming) handlePing(input *packet.Packet) {
	var payload packet.Long
	err := input.Scan(&payload)
	if err != nil {
		return
	}

	c.Incoming.WritePacket(packet.Marshal(
		0x01,
		payload,
	))
}
