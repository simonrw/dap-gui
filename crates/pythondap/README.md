# Python interface to the Debug Adapter Protocol servers

## Design

This program treats debugging a little differently to most. This project treats the program as a variable with properties:

* `variables`
* `stack`

The user is then expected to call methods on the program to manage the execution:

* `program.resume()` to continue execution
* `program.step_over()` to execute the next step
* `program.step_in()`
* `program.step_out()`
* `program.go_to(...)` to run to another step

Custom behaviour can be added as arbitrary Python code with the `add_hook` method, for example:

```python
def my_hook(program: Program):
    print(program.variables["a"])


program.add_hook(my_hook)
program.resume()
# when breakpoint is hit, the variable "a" will be printed
```

