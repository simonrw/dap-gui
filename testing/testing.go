package testing

import (
	"os"
	"path"
	"runtime"
)

func init() {
	_, filename, _, _ := runtime.Caller(0)
	dir := path.Join(path.Dir(filename), "..")
	if err := os.Chdir(dir); err != nil {
		panic(err)
	}
}
