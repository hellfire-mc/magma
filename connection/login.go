package connection

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

func (c *Incoming) handleLoginState(input *packet.Packet) {
	switch input.ID {
	case 0x00:
		c.handleLogin(input)
	case 0x01:
		c.handleEncryption(input)
	}
}

func (c *Incoming) handleLogin(input *packet.Packet) {
	var username packet.String
	err := input.Scan(&username)
	if err != nil {
		return
	}
	c.Player.Username = string(username)

	log.Info().Str("username", c.Player.Username).Msg("Got login request")

	encodedKey, err := x509.MarshalPKIXPublicKey(&c.CM.EncryptionKey.PublicKey)
	if err != nil {
		log.Err(err).Msg("Failed to marshal public key")
		c.Connection.Close()
		return
	}
	verifyString := []byte(utils.RandString(4))
	c.VerifyToken = verifyString
	var out []byte
	out = append(out, packet.String(c.CM.ServerID).Encode()...)
	out = append(out, packet.VarInt(int32(len(encodedKey))).Encode()...)
	out = append(out, encodedKey...)
	out = append(out, packet.VarInt(int32(len(verifyString))).Encode()...)
	out = append(out, verifyString...)

	_ = c.Connection.WritePacket(packet.Packet{
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

func (c *Incoming) handleEncryption(input *packet.Packet) {
	var er encryptionResponse
	if err := input.Scan(&er); err != nil {
		return
	}

	sharedSecret, err := rsa.DecryptPKCS1v15(rand.Reader, c.CM.EncryptionKey, er.SharedSecret)
	if err != nil {
		log.Err(err).Msg("Failed to decrypt shared secret")
		c.Connection.Close()
		return
	}
	verifyToken, err := rsa.DecryptPKCS1v15(rand.Reader, c.CM.EncryptionKey, er.VerifyToken)
	if err != nil {
		log.Err(err).Msg("Failed to decrypt verify token")
		c.Connection.Close()
		return
	}

	if string(verifyToken) != string(c.VerifyToken) {
		log.Error().Msg("Verify token doesn't match!")
		c.Connection.Close()
		return
	}

	// x509 public key
	encodedKey, err := x509.MarshalPKIXPublicKey(&c.CM.EncryptionKey.PublicKey)
	if err != nil {
		log.Err(err).Msg("Failed to marshal public key")
		c.Connection.Close()
		return
	}

	// Handle server auth
	authHash := utils.AuthDigest(c.CM.ServerID, sharedSecret, encodedKey)

	joinURL := fmt.Sprintf(
		"https://sessionserver.mojang.com/session/minecraft/hasJoined?username=%s&serverId=%s",
		c.Player.Username,
		authHash,
	)
	log.Info().Msg("Validating authentication with Mojang")
	PostRequest, err := http.NewRequest(http.MethodGet, joinURL, nil)
	if err != nil {
		log.Error().Msg("unable to create request")
		c.Connection.Close()
		return
	}
	client := http.Client{}
	PostRequest.Header.Set("User-agent", "go-mc")
	PostRequest.Header.Set("Connection", "keep-alive")
	resp, err := client.Do(PostRequest)
	if err != nil {
		log.Error().Msg("auth request failed")
		c.Connection.Close()
		return
	}
	defer resp.Body.Close()
	body, _ := ioutil.ReadAll(resp.Body)
	if resp.StatusCode != 200 {
		log.Error().Msg("auth failed")
		c.SendDisconnect("bad auth")
		return
	}

	var ar AuthResponse
	err = json.Unmarshal(body, &ar)
	if err != nil {
		log.Error().Msg("auth response bad")
		c.Connection.Close()
		return
	}

	id, err := uuid.Parse(ar.ID)
	if err != nil {
		log.Error().Msg("bad uuid")
		c.Connection.Close()
		return
	}

	// Enable encryption
	encoStream, decoStream := utils.NewSymmetricEncryption(sharedSecret)
	c.Connection.SetCipher(encoStream, decoStream)

	c.Player.UUID = id.String()

	log.Info().Str("username", c.Player.Username).Str("id", c.Player.UUID).Msg("Auth successful - opening proxy connection...")
	
	

	// Change state to Play
	c.ChangeState(constants.Play)
	c.CM.AddPlayer(c.Player)
}
