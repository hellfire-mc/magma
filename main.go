package main

import (
	"crypto/rand"
	"crypto/rsa"
	"fmt"
	"github.com/Tnze/go-mc/net"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
	"github.com/skyezerfox/moss/connection"
	"github.com/skyezerfox/moss/constants"
	"github.com/skyezerfox/moss/models"
	"github.com/skyezerfox/moss/utils"
	"github.com/spf13/viper"
	"os"
)

func init() {
	log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stderr})

	viper.SetConfigName("moss")
	viper.SetConfigType("yaml")
	viper.AddConfigPath(".")

	viper.SetDefault("listener.host", "localhost")
	viper.SetDefault("listener.port", 25565)

	viper.SetDefault("server.max_players", 100)
	viper.SetDefault("server.fake_players", -1)
	viper.SetDefault("server.motd", "Just another Mossball proxy!")

	viper.SetDefault("proxy.host", "localhost")
	viper.SetDefault("proxy.port", 25566)

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
}

func main() {
	log.Info().Int("port", viper.GetInt("listener.port")).Msg("Starting proxy server...")

	listener, err := net.ListenMC(fmt.Sprintf(":%d", viper.GetInt("listener.port")))
	if err != nil {
		log.Fatal().Msg(fmt.Sprintf("Unable to listen on port %d", viper.GetInt("listener.port")))
	}

	log.Debug().Msg("Generating encryption key pair...")
	reader := rand.Reader
	encryptionKey, err := rsa.GenerateKey(reader, constants.KeySize)
	if err != nil {
		log.Fatal().Msg("Unable to generate RSA key pair")
		os.Exit(1)
	}

	cm := connection.NewConnectionManager(utils.RandString(16), encryptionKey)

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
