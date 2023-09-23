package client

import (
	"bufio"
	"errors"
	"fmt"
	"net"
	"os"
	"os/exec"
	"strings"
	"testing"
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

	callbackErr := f(0)

	if err := child.Process.Kill(); err != nil {
		t.Fatal(err)
	}

	if callbackErr != nil {
		t.Fatal(callbackErr)
	}
}

func TestFoo(t *testing.T) {
	withServer(t, func(port int) error {
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
