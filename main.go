package main

import (
	"bufio"
	"log"
	"net"
	"time"

	dap "github.com/google/go-dap"
)

func main() {
	pm := dap.ProtocolMessage{
		Seq: 1,
		Type: "request",
	}
	r := dap.Request{
		ProtocolMessage: pm,
		Command:         "initialize",
	}
	i := dap.InitializeRequest{
		Request: r,
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
