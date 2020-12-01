package connection

import (
	"sync"
)

// Player represents a player connected to the proxy.
type Player struct {
	sync.Mutex

	Incoming Incoming
	Outgoing Outgoing
	CM         *ConnectionManager

	State    int
	UUID     string
	Username string

}
