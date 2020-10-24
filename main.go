package main

import (
	"crypto/rand"
	"crypto/rsa"
	"fmt"
	"github.com/Tnze/go-mc/net"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
	"github.com/skyezerfox/moss/constants"
	"github.com/skyezerfox/moss/models"
	"github.com/skyezerfox/moss/protocol"
	"github.com/skyezerfox/moss/utils"
	"github.com/spf13/viper"
	"os"
)

func init() {
	log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stderr})

	viper.SetConfigName("config")
	viper.SetConfigType("toml")
	viper.AddConfigPath(".")

	viper.SetDefault("connection.address", "127.0.0.1")
	viper.SetDefault("connection.port", 25565)
	viper.SetDefault("connection.maxPlayers", 150)

	viper.SetDefault("info.motd", "Just a GoLang Minecraft Server")
	viper.SetDefault("info.serverIcon", "icon.jpg")

	viper.SetDefault("servers.lobby.host", "127.0.0.1")
	viper.SetDefault("servers.lobby.port", 25566)

	if err := viper.ReadInConfig(); err != nil {
		if _, ok := err.(viper.ConfigFileNotFoundError); ok {
			err = viper.SafeWriteConfig()
			if err != nil {
				log.Fatal().Msg("Failed to write sample config")
				os.Exit(1)
			}
		} else {
			log.Fatal().Msg("Failed to read config")
			os.Exit(1)
		}
	}

	// deregister defaults so they don't accidentally existcm := protocol.NewConnectionManager(encryptionKey)
	viper.SetDefault("servers.lobby.host", nil)
	viper.SetDefault("servers.lobby.port", nil)
}

func main() {
	log.Info().Int("port", viper.GetInt("connection.port")).Msg("Starting proxy server...")

	listener, err := net.ListenMC(fmt.Sprintf(":%d", viper.GetInt("connection.port")))
	if err != nil {
		log.Fatal().Msg(fmt.Sprintf("Unable to listen on port %d", viper.GetInt("connection.port")))
	}

	log.Debug().Msg("Generating encryption key pair...")
	reader := rand.Reader
	encryptionKey, err := rsa.GenerateKey(reader, constants.KeySize)
	if err != nil {
		log.Fatal().Msg("Unable to generate RSA key pair")
		os.Exit(1)
	}

	cm := protocol.NewConnectionManager(utils.RandString(16), encryptionKey)

	// read server configuration
	for k, s := range viper.GetStringMap("servers") {
		switch v := s.(type) {
		case map[string]interface{}:
			if v["host"] == nil {
				log.Fatal().Msgf("Invalid configuration block for server %s - missing host config", k)
				continue
			} else if v["port"] == nil {
				log.Fatal().Msgf("Invalid configuration block for server %s - missing port config", k)
				continue
			}
			cm.AddServer(&models.ServerConfig{
				Name: k,
				Host: v["host"].(string),
				Port: int(v["port"].(int64)),
			})
		default:
			log.Fatal().Msgf("Invalid configuration block for server %s", k)
		}
	}

	log.Debug().Msgf("Have %d servers available for proxying", cm.GetServerCount())

	for {
		conn, err := listener.Accept()
		if err != nil {
			continue
		}

		c := cm.NewConnection(&conn)
		go c.HandlePlayer()
	}
}
