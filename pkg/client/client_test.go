package client

import (
	"bufio"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"os"
	"os/exec"
	"strings"
	"testing"

	"github.com/google/go-dap"
)

func getRandomTCPPort() (int, error) {
	for i := 0; i < 50; i++ {
		conn, err := net.Listen("tcp", "127.0.0.1:0")
		if err != nil {
			// fatal error?
			continue
		}
		defer conn.Close()
		addr := conn.Addr()
		tcpAddr, ok := addr.(*net.TCPAddr)
		if !ok {
			// fatal error?
			continue
		}
		return tcpAddr.Port, nil
	}

	return 0, errors.New("bad")
}

func withServer(t *testing.T, f func(int) error) {
	port, err := getRandomTCPPort()
	if err != nil {
		t.Fatal(err)
	}

	cwd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}

	child := exec.Command("python", "-m", "debugpy.adapter", "--host", "127.0.0.1", "--port", fmt.Sprintf("%d", port), "--log-stderr")
	child.Dir = cwd
	childout, _ := child.StderrPipe()

	if err := child.Start(); err != nil {
		t.Fatal(err)
	}

	c := make(chan struct{}, 1)
	go func() {
		scanner := bufio.NewScanner(childout)
		scanner.Split(bufio.ScanLines)
		shouldSignal := true
		for scanner.Scan() {
			m := scanner.Text()
			t.Log(m)
			if shouldSignal && strings.Contains(m, "Listening for incoming Client connections") {
				c <- struct{}{}
			}
		}
	}()
	// wait for server to be ready
	<-c

	callbackErr := f(port)

	if err := child.Process.Kill(); err != nil {
		t.Fatal(err)
	}

	if callbackErr != nil {
		t.Fatal(callbackErr)
	}
}

func TestFoo(t *testing.T) {
	c := make(chan dap.Message)
	withServer(t, func(port int) error {
		conn, err := net.Dial("tcp", fmt.Sprintf("127.0.0.1:%d", port))
		if err != nil {
			t.Fatal(err)
		}
		defer conn.Close()

		client := New(conn, c)
		go client.Poll()

		// initialise
		initializeR := dap.InitializeRequest{
			Arguments: dap.InitializeRequestArguments{
				AdapterID: "adapter-id",
			},
		}
		if err := client.Send(&initializeR); err != nil {
			t.Fatal(err)
		}

		// launch
		launchArgs := struct {
			Program string `json:"program"`
		}{
			Program: "./test.py",
		}
		launchArgsStr, _ := json.Marshal(launchArgs)
		launchR := dap.LaunchRequest{
			Arguments: launchArgsStr,
		}
		if err := client.Send(&launchR); err != nil {
			t.Fatal(err)
		}

		return nil
	})
}

func TestGetRandomPort(t *testing.T) {
	port, err := getRandomTCPPort()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := net.Listen("tcp", fmt.Sprintf("127.0.0.1:%d", port)); err != nil {
		t.Fatal(err)
	}
}
