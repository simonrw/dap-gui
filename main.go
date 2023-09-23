package main

import (
	"bufio"
	"bytes"
	"fmt"
	"io"
	"log"
	"net"
	"time"

	g "github.com/AllenDang/giu"
	dap "github.com/google/go-dap"
)

func onClickMe() {
	fmt.Println("Hello world!")
}

func onImSoCute() {
	fmt.Println("Im sooooooo cute!!")
}

func loop() {
	g.SingleWindow().Layout(
		g.Label("Hello world from giu"),
		g.Row(
			g.Button("Click Me").OnClick(onClickMe),
			g.Button("I'm so cute").OnClick(onImSoCute),
		),
	)
}

func main() {
	foo()

	// wnd := g.NewMasterWindow("Hello world", 400, 200, g.MasterWindowFlagsNotResizable)
	// wnd.Run(loop)
}

type Client struct {
	seq int
}

func NewClient() *Client {
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

func foo() {
	client := NewClient()
	var b bytes.Buffer
	i := dap.InitializeRequest{
		Arguments: dap.InitializeRequestArguments{
			AdapterID: "adapter-id",
		},
	}
	client.Send(&b, &i)
	client.Send(&b, &i)

	fmt.Printf("%s\n", b.String())
}

func mainClient() {
	// pm := dap.ProtocolMessage{
	// 	Seq:  1,
	// 	Type: "request",
	// }
	// r := dap.Request{
	// 	ProtocolMessage: pm,
	// 	Command:         "initialize",
	// }
	i := dap.InitializeRequest{
		// Request: r,
		Arguments: dap.InitializeRequestArguments{
			AdapterID: "adapter-id",
		},
	}
	client, err := net.Dial("tcp", "127.0.0.1:5678")
	if err != nil {
		panic(err)
	}
	defer client.Close()

	go func() {
		reader := bufio.NewReader(client)
		// handle background messages
		for {
			msg, err := dap.ReadProtocolMessage(reader)
			if err != nil {
				log.Printf("reading message from client: %v", err)
				continue
			}
			log.Printf("message: %+v", msg)
		}
	}()

	if err := dap.WriteProtocolMessage(client, &i); err != nil {
		panic(err)
	}

	time.Sleep(10 * time.Second)
}
