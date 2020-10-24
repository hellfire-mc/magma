package protocol

import (
	"crypto/aes"
	"crypto/cipher"
	"github.com/Tnze/go-mc/net/CFB8"
)

func newSymmetricEncryption(key []byte) (encoStream, decoStream cipher.Stream) {
	b, err := aes.NewCipher(key)
	if err != nil {
		panic("Unable to generate new cipher")
	}

	decoStream = CFB8.NewCFB8Decrypt(b, key)
	encoStream = CFB8.NewCFB8Encrypt(b, key)
	return
}
