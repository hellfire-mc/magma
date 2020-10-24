package models

type (
	ServerStatus struct {
		Version     Version     `json:"version"`
		Players     Players     `json:"players"`
		Description Description `json:"description"`
		Favicon     string      `json:"favicon"`
	}

	Version struct {
		Name     string `json:"name"`
		Protocol int    `json:"protocol"`
	}

	Players struct {
		Max    int      `json:"max"`
		Online int      `json:"online"`
		Sample []Sample `json:"sample"`
	}

	Description struct {
		Text string `json:"text"`
	}

	Sample struct {
		Name string `json:"name"`
		ID   string `json:"id"`
	}
)
