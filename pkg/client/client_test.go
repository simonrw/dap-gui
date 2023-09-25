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
	_ "github.com/simonrw/dap-gui/testing"
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

func TestEndToEnd(t *testing.T) {
	c := make(chan dap.Message)
	withServer(t, func(port int) error {
		t.Logf("connecting to server on port %d", port)
		conn, err := net.Dial("tcp", fmt.Sprintf("127.0.0.1:%d", port))
		if err != nil {
			t.Fatal(err)
		}
		defer conn.Close()

		client := New(conn, c)
		t.Log("starting poll loop in the background")
		go client.Poll()

		// initialise
		initializeR := dap.InitializeRequest{
			Arguments: dap.InitializeRequestArguments{
				AdapterID: "adapter-id",
			},
		}
		t.Logf("sending initialize event %+v", initializeR)
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
		t.Logf("sending launch event %+v", launchR)
		if err := client.Send(&launchR); err != nil {
			t.Fatal(err)
		}

		t.Log("waiting for initialized event")
		if _, err := waitFor(c, func(msg dap.Message) bool {
			_, ok := msg.(*dap.InitializedEvent)
			return ok
		}); err != nil {
			t.Fatal(err)
		}
		t.Logf("received initialized event")

		brkR := dap.SetFunctionBreakpointsRequest{
			Arguments: dap.SetFunctionBreakpointsArguments{
				Breakpoints: []dap.FunctionBreakpoint{
					{
						Name: "main",
					},
				},
			},
		}
		t.Logf("setting function breakpoints %+v", brkR)
		if err := client.Send(&brkR); err != nil {
			t.Fatal(err)
		}

		t.Log("waiting for set function breakpoints response")
		brkRes, err := waitFor(c, func(msg dap.Message) bool {
			_, ok := msg.(*dap.SetFunctionBreakpointsResponse)
			return ok
		})
		if err != nil {
			t.Fatal(err)
		}
		t.Logf("got function breakpoints response: %+v", brkRes)

		cDoneR := dap.ConfigurationDoneRequest{}
		t.Logf("configuration done: %+v", cDoneR)
		if err := client.Send(&cDoneR); err != nil {
			t.Fatal(err)
		}
		if _, err := waitFor(c, func(msg dap.Message) bool {
			_, ok := msg.(*dap.ConfigurationDoneResponse)
			return ok
		}); err != nil {
			t.Fatal(err)
		}

		// wait for stopped event
		stoppedERaw, err := waitFor(c, func(msg dap.Message) bool {
			_, ok := msg.(*dap.StoppedEvent)
			return ok
		})
		if err != nil {
			t.Fatal(err)
		}
		stoppedE := stoppedERaw.(*dap.StoppedEvent)
		t.Logf("got stopped event %+v", stoppedE)

		// fetch thread info
		t.Log("getting threads")
		if err := client.Send(&dap.ThreadsRequest{}); err != nil {
			t.Fatal(err)
		}
		if _, err := waitFor(c, func(msg dap.Message) bool {
			_, ok := msg.(*dap.ThreadsResponse)
			return ok
		}); err != nil {
			t.Fatal(err)
		}

		// fetch stack info
		t.Log("fetching stack info")
		if err := client.Send(&dap.StackTraceRequest{
			Arguments: dap.StackTraceArguments{
				ThreadId: stoppedE.Body.ThreadId,
			},
		}); err != nil {
			t.Fatal(err)
		}
		traceRaw, err := waitFor(c, func(msg dap.Message) bool {
			_, ok := msg.(*dap.StackTraceResponse)
			return ok
		})
		traceRes := traceRaw.(*dap.StackTraceResponse)

		for _, frame := range traceRes.Body.StackFrames {
			t.Logf("fetching scopes for frame %s (%d)", frame.Name, frame.Id)
			if err := client.Send(&dap.ScopesRequest{
				Arguments: dap.ScopesArguments{
					FrameId: frame.Id,
				},
			}); err != nil {
				t.Fatal(err)
			}

			scopesRaw, err := waitFor(c, func(msg dap.Message) bool {
				_, ok := msg.(*dap.ScopesResponse)
				return ok
			})
			if err != nil {
				t.Fatal(err)
			}
			scopesRes := scopesRaw.(*dap.ScopesResponse)

			for _, scope := range scopesRes.Body.Scopes {
				t.Logf("fetching variables for scope %s (%d)", scope.Name, scope.VariablesReference)
				if err := client.Send(&dap.VariablesRequest{
					Arguments: dap.VariablesArguments{
						VariablesReference: scope.VariablesReference,
					},
				}); err != nil {
					t.Fatal(err)
				}

				if _, err := waitFor(c, func(msg dap.Message) bool {
					_, ok := msg.(*dap.VariablesResponse)
					return ok
				}); err != nil {
					t.Fatal(err)
				}
			}

		}

		t.Logf("sending continue request")
		if err := client.Send(&dap.ContinueRequest{
			Arguments: dap.ContinueArguments{
				ThreadId:     stoppedE.Body.ThreadId,
				SingleThread: false,
			},
		}); err != nil {
			t.Fatal(err)
		}
		if _, err := waitFor(c, func(msg dap.Message) bool {
			_, ok := msg.(*dap.ContinueResponse)
			return ok
		}); err != nil {
			t.Fatal(err)
		}

		t.Log("waiting for terminated event")
		if _, err := waitFor(c, func(msg dap.Message) bool {
			_, ok := msg.(*dap.TerminatedEvent)
			return ok
		}); err != nil {
			t.Fatal(err)
		}

		t.Log("terminate")
		if err := client.Send(&dap.TerminateRequest{
			Arguments: &dap.TerminateArguments{
				Restart: false,
			},
		}); err != nil {
			t.Fatal(err)
		}
		if _, err := waitFor(c, func(msg dap.Message) bool {
			_, ok := msg.(*dap.TerminateResponse)
			return ok
		}); err != nil {
			t.Fatal(err)
		}

		t.Log("disconnect")
		if err := client.Send(&dap.DisconnectRequest{
			Arguments: &dap.DisconnectArguments{
				TerminateDebuggee: true,
			},
		}); err != nil {
			t.Fatal(err)
		}
		if _, err := waitFor(c, func(msg dap.Message) bool {
			_, ok := msg.(*dap.DisconnectResponse)
			return ok
		}); err != nil {
			t.Fatal(err)
		}
		return nil
	})
}

func waitFor(ch chan dap.Message, pred func(dap.Message) bool) (dap.Message, error) {
	n := 0
	for {
		if n >= 10 {
			break
		}
		msg := <-ch

		if _, ok := msg.(*dap.OutputEvent); ok {
			continue
		}

		if pred(msg) {
			return msg, nil
		}

		n++
	}
	return nil, fmt.Errorf("did not receive message")
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
