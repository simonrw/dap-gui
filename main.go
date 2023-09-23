package main

import (
	"fmt"
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

func mainGui() {
	wnd := g.NewMasterWindow("Hello world", 400, 200, g.MasterWindowFlagsNotResizable)
	wnd.Run(loop)
}

func main() {
	conn, err := net.Dial("tcp", "127.0.0.1:5678")
	if err != nil {
		panic(err)
	}
	defer conn.Close()

	c := client.New(conn)
	// TODO defer c.Shutdown()
	go c.Poll()

	i := dap.InitializeRequest{
		Arguments: dap.InitializeRequestArguments{
			AdapterID: "adapter-id",
		},
	}

	if err := c.Send(&i); err != nil {
		panic(err)
	}

	time.Sleep(10 * time.Second)
}
