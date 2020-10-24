package protocol

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"encoding/json"
	"fmt"
	"github.com/Tnze/go-mc/net/packet"
	"github.com/google/uuid"
	"github.com/skyezerfox/moss/constants"
	"github.com/skyezerfox/moss/utils"
	"io/ioutil"
	"net/http"
	"github.com/rs/zerolog/log"
)

type AuthResponse struct {
	ID         string `json:"id"`
	Name       string `json:"name"`
	Properties []struct {
		Name      string `json:"name"`
		Value     string `json:"value"`
		Signature string `json:"signature"`
	} `json:"properties"`
}

func (c *Connection) handleLoginState(input *packet.Packet) {
	switch input.ID {
	case 0x00:
		c.handleLogin(input)
	case 0x01:
		c.handleEncryption(input)
	}
}

func (c *Connection) handleLogin(input *packet.Packet) {
	var username packet.String
	err := input.Scan(&username)
	if err != nil {
		return
	}
	c.username = string(username)

	log.Info().Str("username", c.username).Msg("Got login request")

	encodedKey, err := x509.MarshalPKIXPublicKey(&c.cm.encryptionKey.PublicKey)
	if err != nil {
		log.Err(err).Msg("Failed to marshal public key")
		c.conn.Close()
		return
	}
	verifyString := []byte(utils.RandString(4))
	c.verifyToken = verifyString
	var out []byte
	out = append(out, packet.String(c.cm.serverID).Encode()...)
	out = append(out, packet.VarInt(int32(len(encodedKey))).Encode()...)
	out = append(out, encodedKey...)
	out = append(out, packet.VarInt(int32(len(verifyString))).Encode()...)
	out = append(out, verifyString...)

	_ = c.conn.WritePacket(packet.Packet{
		ID:   0x01,
		Data: out,
	})
}

type encryptionResponse struct {
	SharedSecret []byte
	VerifyToken  []byte
}

func (e *encryptionResponse) Decode(r packet.DecodeReader) error {
	var secretLength, verifyTokenLength packet.VarInt
	if err := secretLength.Decode(r); err != nil {
		return err
	}
	sharedSecret, err := packet.ReadNBytes(r, int(secretLength))
	if err != nil {
		return err
	}

	if err := verifyTokenLength.Decode(r); err != nil {
		return err
	}

	verifyToken, err := packet.ReadNBytes(r, int(verifyTokenLength))
	if err != nil {
		return err
	}

	e.SharedSecret = sharedSecret
	e.VerifyToken = verifyToken
	return nil
}

func (c *Connection) handleEncryption(input *packet.Packet) {
	var er encryptionResponse
	if err := input.Scan(&er); err != nil {
		return
	}

	sharedSecret, err := rsa.DecryptPKCS1v15(rand.Reader, c.cm.encryptionKey, er.SharedSecret)
	if err != nil {
		log.Err(err).Msg("Failed to decrypt shared secret")
		c.conn.Close()
		return
	}
	verifyToken, err := rsa.DecryptPKCS1v15(rand.Reader, c.cm.encryptionKey, er.VerifyToken)
	if err != nil {
		log.Err(err).Msg("Failed to decrypt verify token")
		c.conn.Close()
		return
	}

	if string(verifyToken) != string(c.verifyToken) {
		log.Error().Msg("Verify token doesn't match!")
		c.conn.Close()
		return
	}

	// x509 public key
	encodedKey, err := x509.MarshalPKIXPublicKey(&c.cm.encryptionKey.PublicKey)
	if err != nil {
		log.Err(err).Msg("Failed to marshal public key")
		c.conn.Close()
		return
	}

	// Handle server auth
	authHash := utils.AuthDigest(c.cm.serverID, sharedSecret, encodedKey)

	joinURL := fmt.Sprintf(
		"https://sessionserver.mojang.com/session/minecraft/hasJoined?username=%s&serverId=%s",
		c.username,
		authHash,
	)
	log.Info().Msg("Validating authentication with Mojang")
	PostRequest, err := http.NewRequest(http.MethodGet, joinURL, nil)
	if err != nil {
		log.Error().Msg("unable to create request")
		c.conn.Close()
		return
	}
	client := http.Client{}
	PostRequest.Header.Set("User-agent", "go-mc")
	PostRequest.Header.Set("Connection", "keep-alive")
	resp, err := client.Do(PostRequest)
	if err != nil {
		log.Error().Msg("auth request failed")
		c.conn.Close()
		return
	}
	defer resp.Body.Close()
	body, _ := ioutil.ReadAll(resp.Body)
	if resp.StatusCode != 200 {
		log.Error().Msg("auth failed")
		c.sendDisconnect("bad auth")
		return
	}

	var ar AuthResponse
	err = json.Unmarshal(body, &ar)
	if err != nil {
		log.Error().Msg("auth response bad")
		c.conn.Close()
		return
	}

	id, err := uuid.Parse(ar.ID)
	if err != nil {
		log.Error().Msg("bad uuid")
		c.conn.Close()
		return
	}

	// Enable encryption
	encoStream, decoStream := newSymmetricEncryption(sharedSecret)
	c.conn.SetCipher(encoStream, decoStream)

	c.uuid = id.String()
	log.Info().Str("username", c.username).Str("id", c.uuid).Msg("Login successful")
	
	// Send login success
	c.conn.WritePacket(packet.Marshal(
		0x02,
		packet.UUID(id),
		packet.String(c.username),
	))

	// Change state to Play
	c.changeState(constants.Play)
	c.cm.AddPlayer(c)

	// c.sendJoinData(input)
}
