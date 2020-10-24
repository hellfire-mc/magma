package protocol

import (
	"crypto/rsa"
	"github.com/Tnze/go-mc/net"
	"github.com/skyezerfox/moss/constants"
	"github.com/skyezerfox/moss/models"
	"sync"
	// "sync/atomic"
)

// ClientManager Manages connected players.
type ConnectionManager struct {
	sync.Mutex
	players []*Connection
	servers []*models.ServerConfig

	currentID     *int32
	serverID      string
	encryptionKey *rsa.PrivateKey
}


func NewConnectionManager(serverID string, key *rsa.PrivateKey) *ConnectionManager {
	initialID := int32(0)
	return &ConnectionManager{
		players:       make([]*Connection, 0),
		currentID:     &initialID,
		serverID:      serverID,
		encryptionKey: key,
	}
}

// NewClient Create a new client
func (cm *ConnectionManager) NewConnection(conn *net.Conn) *Connection {
	c := &Connection{
		conn:  conn,
		state: constants.Handshaking,
		cm:    cm,
	}
	return c
}

func (cm *ConnectionManager) AddPlayer(c *Connection) {
	cm.Lock()
	defer cm.Unlock()
	cm.players = append(cm.players, c)
}

func remove(s []*Connection, i int) []*Connection {
	s[len(s)-1], s[i] = s[i], s[len(s)-1]
	return s[:len(s)-1]
}

func (cm *ConnectionManager) RemovePlayer(c *Connection) {
	cm.Lock()
	defer cm.Unlock()
	for i, conn := range cm.players {
		if conn == c {
			cm.players = remove(cm.players, i)
			break
		}
	}
}

func (cm *ConnectionManager) AddServer(s *models.ServerConfig) {
	cm.Lock()
	defer cm.Unlock()
	cm.servers = append(cm.servers, s)
}

func (cm *ConnectionManager) GetServerCount() int {
	return len(cm.servers)
}
