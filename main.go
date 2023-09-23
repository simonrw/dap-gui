package main

import (
	"bufio"
	"bytes"
	"fmt"
	"log"
	"net"
	"time"

	g "github.com/AllenDang/giu"
	dap "github.com/google/go-dap"
	"github.com/simonrw/dap-gui/pkg/client"
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

func foo() {
	client := client.New()
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
