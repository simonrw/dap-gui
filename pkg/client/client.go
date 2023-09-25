package client

import (
	"bufio"
	"fmt"
	"io"
	"strings"

	"github.com/google/go-dap"
)

type Client struct {
	conn io.ReadWriter
	seq  int
	ch chan dap.Message
}

func New(conn io.ReadWriter, ch chan dap.Message) *Client {
	return &Client{
		conn: conn,
		seq:  1,
		ch: ch,
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

func (c *Client) Send(r dap.RequestMessage) error {
	switch t := r.(type) {
	case *dap.InitializeRequest:
		t.Request = c.newRequest("initialize")
		return dap.WriteProtocolMessage(c.conn, t)
	case *dap.LaunchRequest:
		t.Request = c.newRequest("launch")
		return dap.WriteProtocolMessage(c.conn, t)
	case *dap.SetFunctionBreakpointsRequest:
		t.Request = c.newRequest("setFunctionBreakpoints")
		return dap.WriteProtocolMessage(c.conn, t)
	case *dap.ConfigurationDoneRequest:
		t.Request = c.newRequest("configurationDone")
		return dap.WriteProtocolMessage(c.conn, t)
	case *dap.ThreadsRequest:
		t.Request = c.newRequest("threads")
		return dap.WriteProtocolMessage(c.conn, t)
	case *dap.StackTraceRequest:
		t.Request = c.newRequest("stackTrace")
		return dap.WriteProtocolMessage(c.conn, t)
	case *dap.ScopesRequest:
		t.Request = c.newRequest("scopes")
		return dap.WriteProtocolMessage(c.conn, t)
	case *dap.VariablesRequest:
		t.Request = c.newRequest("variables")
		return dap.WriteProtocolMessage(c.conn, t)
	case *dap.ContinueRequest:
		t.Request = c.newRequest("continue")
		return dap.WriteProtocolMessage(c.conn, t)
	case *dap.TerminateRequest:
		t.Request = c.newRequest("terminate")
		return dap.WriteProtocolMessage(c.conn, t)
	case *dap.DisconnectRequest:
		t.Request = c.newRequest("disconnect")
		return dap.WriteProtocolMessage(c.conn, t)
	default:
		return fmt.Errorf("unhandled type: %+v", t)
	}
}

func (c *Client) Poll() {
	reader := bufio.NewReader(c.conn)
	for {
		msg, err := dap.ReadProtocolMessage(reader)
		if err != nil {
			if strings.Contains(err.Error(), "use of closed network connection") {
				// the server closed the connection
				return
			}
		}
		c.ch <- msg
	}
}
