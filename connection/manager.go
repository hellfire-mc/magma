package connection

import (
	"crypto/rsa"
	"github.com/Tnze/go-mc/net"
	"github.com/skyezerfox/moss/constants"
	"github.com/skyezerfox/moss/models"
	"sync"
)

// ConnectionManager Manages connected players.
type ConnectionManager struct {
	sync.Mutex
	Players []*Player
	Servers []*models.ServerConfig

	CurrentID     *int32
	ServerID      string
	EncryptionKey *rsa.PrivateKey
}

// NewConnectionManager creates a new connection manager.
func NewConnectionManager(serverID string, key *rsa.PrivateKey) *ConnectionManager {
	initialID := int32(0)
	return &ConnectionManager{
		Players:       make([]*Player, 0),
		CurrentID:     &initialID,
		ServerID:      serverID,
		EncryptionKey: key,
	}
}

// NewConnection creates a new player.
func (cm *ConnectionManager) NewPlayer(conn *net.Conn) *Player {
	player := &Player{
		State:      constants.Handshaking,
		CM:         cm,
	}
	return player
}

// AddPlayer adds a player to ththe connection manager.
func (cm *ConnectionManager) AddPlayer(c *Player) {
	cm.Lock()
	defer cm.Unlock()
	cm.Players = append(cm.Players, c)
}

func remove(s []*Player, i int) []*Player {
	s[len(s)-1], s[i] = s[i], s[len(s)-1]
	return s[:len(s)-1]
}

// RemovePlayer removes a player from the connection manager.
func (cm *ConnectionManager) RemovePlayer(c *Player) {
	cm.Lock()
	defer cm.Unlock()
	for i, conn := range cm.Players {
		if conn == c {
			cm.Players = remove(cm.Players, i)
			break
		}
	}
}

// AddServer adds a server to this connection manager.
func (cm *ConnectionManager) AddServer(s *models.ServerConfig) {
	cm.Lock()
	defer cm.Unlock()
	cm.Servers = append(cm.Servers, s)
}

// GetServerCount returns the number of servers currently proxied.
func (cm *ConnectionManager) GetServerCount() int {
	return len(cm.Servers)
}
