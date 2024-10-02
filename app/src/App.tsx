// Generated via https://v0.dev/chat/QJ66R2ulISS?b=b_G9oPm4yTYz9&p=0

import React, { useState } from 'react'
import { ChevronRight, ChevronDown, Play, Pause, Terminal } from 'lucide-react'

// Mock data
const mockCode = `function fibonacci(n) {
  if (n <= 1) return n;
  return fibonacci(n - 1) + fibonacci(n - 2);
}

function main() {
  const result = fibonacci(10);
  console.log(result);
}

main();`

const mockVariables = [
  { name: 'n', type: 'number', value: '10' },
  { name: 'result', type: 'number', value: '55' },
]

const mockBreakpoints = [
  { id: 1, filename: 'fibonacci.js', line: 2, enabled: true },
  { id: 2, filename: 'fibonacci.js', line: 7, enabled: false },
]

const mockStackFrames = [
  { id: 1, functionName: 'fibonacci', filename: 'fibonacci.js', line: 2 },
  { id: 2, functionName: 'main', filename: 'fibonacci.js', line: 7 },
]

export default function DebuggerUI() {
  const [consoleInput, setConsoleInput] = useState('')
  const [consoleOutput, setConsoleOutput] = useState<string[]>([])

  const handleConsoleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    setConsoleOutput([...consoleOutput, `> ${consoleInput}`])
    setConsoleInput('')
  }

  return (
    <div className="flex flex-col h-screen bg-gray-100 text-gray-800">
      <header className="bg-gray-800 text-white p-4">
        <h1 className="text-2xl font-bold">Debugger</h1>
      </header>
      <div className="flex-1 grid grid-cols-3 gap-4 p-4">
        <div className="col-span-2 flex flex-col gap-4">
          <CodeView code={mockCode} currentLine={2} breakpoints={[2, 7]} />
          <Console
            input={consoleInput}
            output={consoleOutput}
            onInputChange={setConsoleInput}
            onSubmit={handleConsoleSubmit}
          />
        </div>
        <div className="flex flex-col gap-4">
          <VariablesView variables={mockVariables} />
          <BreakpointsList breakpoints={mockBreakpoints} />
          <StackFramesList stackFrames={mockStackFrames} />
        </div>
      </div>
    </div>
  )
}

function CodeView({ code, currentLine, breakpoints }) {
  const lines = code.split('\n')

  return (
    <div className="bg-white rounded-lg shadow overflow-hidden">
      <div className="flex items-center justify-between bg-gray-200 px-4 py-2">
        <h2 className="font-semibold">Code View</h2>
        <div className="flex gap-2">
          <button className="p-1 rounded hover:bg-gray-300">
            <Play size={16} />
          </button>
          <button className="p-1 rounded hover:bg-gray-300">
            <Pause size={16} />
          </button>
        </div>
      </div>
      <div className="p-4 font-mono text-sm">
        {lines.map((line, index) => (
          <div
            key={index}
            className={`flex ${currentLine === index + 1 ? 'bg-yellow-100' : ''}`}
          >
            <div className="w-8 text-right pr-2 text-gray-500 select-none">
              {breakpoints.includes(index + 1) && (
                <span className="text-red-500">●</span>
              )}
              {index + 1}
            </div>
            <pre className="flex-1">{line}</pre>
          </div>
        ))}
      </div>
    </div>
  )
}

function VariablesView({ variables }) {
  return (
    <div className="bg-white rounded-lg shadow">
      <div className="bg-gray-200 px-4 py-2">
        <h2 className="font-semibold">Variables</h2>
      </div>
      <div className="p-4">
        {variables.map((variable, index) => (
          <div key={index} className="flex justify-between py-1">
            <span>{variable.name}</span>
            <span className="text-gray-500">
              {variable.type}: {variable.value}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

function BreakpointsList({ breakpoints }) {
  return (
    <div className="bg-white rounded-lg shadow">
      <div className="bg-gray-200 px-4 py-2">
        <h2 className="font-semibold">Breakpoints</h2>
      </div>
      <div className="p-4">
        {breakpoints.map((breakpoint) => (
          <div key={breakpoint.id} className="flex items-center justify-between py-1">
            <span>
              {breakpoint.filename}:{breakpoint.line}
            </span>
            <button
              className={`px-2 py-1 rounded ${
                breakpoint.enabled ? 'bg-green-500 text-white' : 'bg-gray-300'
              }`}
            >
              {breakpoint.enabled ? 'Enabled' : 'Disabled'}
            </button>
          </div>
        ))}
      </div>
    </div>
  )
}

function StackFramesList({ stackFrames }) {
  return (
    <div className="bg-white rounded-lg shadow">
      <div className="bg-gray-200 px-4 py-2">
        <h2 className="font-semibold">Stack Frames</h2>
      </div>
      <div className="p-4">
        {stackFrames.map((frame) => (
          <div key={frame.id} className="py-1 cursor-pointer hover:bg-gray-100">
            <div className="font-semibold">{frame.functionName}</div>
            <div className="text-sm text-gray-500">
              {frame.filename}:{frame.line}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}

function Console({ input, output, onInputChange, onSubmit }) {
  return (
    <div className="bg-white rounded-lg shadow">
      <div className="flex items-center bg-gray-200 px-4 py-2">
        <Terminal size={16} className="mr-2" />
        <h2 className="font-semibold">Console</h2>
      </div>
      <div className="p-4 h-40 overflow-y-auto font-mono text-sm">
        {output.map((line, index) => (
          <div key={index}>{line}</div>
        ))}
      </div>
      <form onSubmit={onSubmit} className="p-4 border-t">
        <input
          type="text"
          value={input}
          onChange={(e) => onInputChange(e.target.value)}
          className="w-full px-2 py-1 border rounded"
          placeholder="Enter command..."
        />
      </form>
    </div>
  )
}
