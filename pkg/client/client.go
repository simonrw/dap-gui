package client

import (
	"fmt"
	"io"

	"github.com/google/go-dap"
)

type Client struct {
	seq int
}

func New() *Client {
	return &Client{
		seq: 1,
	}
}

func (c *Client) newRequest(command string) dap.Request {
	seq := c.seq
	c.seq++
	pm := dap.ProtocolMessage{
		Seq:  seq,
		Type: "request",
	}
	r := dap.Request{
		ProtocolMessage: pm,
		Command:         command,
	}
	return r
}

func (c *Client) Send(w io.Writer, r dap.RequestMessage) error {
	switch t := r.(type) {
	case *dap.InitializeRequest:
		t.Request = c.newRequest("initialize")
		return dap.WriteProtocolMessage(w, t)
	default:
		return fmt.Errorf("unhandled type: %v", t)
	}
}
