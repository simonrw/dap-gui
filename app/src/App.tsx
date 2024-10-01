import { ArrowDownToDot, Circle, RedoDot, StepForward } from "lucide-react";
import { Button } from "./components/ui/button";
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "./components/ui/resizable"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "./components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./components/ui/tabs";
import { Textarea } from "./components/ui/textarea";
import Terminal from "react-console-emulator";
import { ReactNode, useCallback, useMemo, useState } from "react";
import ReactCodeMirror, { EditorView, gutter, GutterMarker, lineNumbers, RangeSet, StateEffect, StateField } from "@uiw/react-codemirror";
import { javascript } from "@codemirror/lang-javascript";
import { githubDark } from "@uiw/codemirror-theme-github";

function Controls() {
  return (
    <div className="flex justify-center">
      <Button size="icon">
        <StepForward />
      </Button>
      <Button size="icon">
        <RedoDot />
      </Button>
      <Button size="icon">
        <ArrowDownToDot />
      </Button>
    </div>
  );
}

type Breakpoint = {
  file: string;
  lineNumber: string;
  functionName: string | null;
  enabled: boolean;
};

type BreakpointStatusProps = {
  value: boolean;
};

function BreakpointStatus(props: BreakpointStatusProps) {
  const cls = props.value ? "text-red-500" : "text-gray-500";
  return (
    <span className={cls}>
      <Circle />
    </span>
  );
}

function Breakpoints() {
  const breakpoints: Breakpoint[] = [
    {
      file: "test.py",
      lineNumber: "167",
      functionName: null,
      enabled: true,
    }
  ];
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Breakpoints</TableHead>
          <TableHead></TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {breakpoints.map((breakpoint) => (
          <TableRow key={`${breakpoint.file}:${breakpoint.lineNumber}`}>
            <TableCell className="font-medium">{`${breakpoint.file}:${breakpoint.lineNumber}`}</TableCell>
            <TableCell className="font-medium">{
              <BreakpointStatus value={breakpoint.enabled} />
            }</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table >
  );
}

type StackFrame = {
  functionName: string;
};

function StackFrame(props: { stackFrame: StackFrame }) {
  const { stackFrame } = props;

  return (
    <TableRow key={stackFrame.functionName}>
      <TableCell className="font-medium">{stackFrame.functionName}</TableCell>
    </TableRow>
  );
}

function StackList() {
  const stackFrames: StackFrame[] = ["my_function", "another_function"].map((name) => {
    return { functionName: name };
  });
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Stack</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {stackFrames.map((stackFrame) => <StackFrame key={stackFrame.functionName} stackFrame={stackFrame} />)}
      </TableBody>
    </Table >
  );
}


type Variable = {
  name: string;
  value: string;
  type: string;
};
function Variables() {
  const variables: Variable[] = [
    { name: "a", value: "10", type: "int" },
    { name: "b", value: "100", type: "int" },
    { name: "c", value: "foobar", type: "string" },
  ];

  return (
    <div className="flex items-center justify-center">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="w-[100px]">Name</TableHead>
            <TableHead className="w-[100px]">Type</TableHead>
            <TableHead className="">Value</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {variables.map((variable) => (
            <TableRow key={variable.name}>
              <TableCell className="font-medium">{variable.name}</TableCell>
              <TableCell className="">{variable.type}</TableCell>
              <TableCell className="">{variable.value}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

const terminalCommands = {
  echo: {
    description: "Echo a string",
    usage: "echo <string>",
    fn: (...args: string[]) => args.join(" "),
  },
};

type DebugTerminalProps = {
}

function DebugTerminal(props: DebugTerminalProps) {
  return (
    <div id="terminal">
      <Terminal
        commands={terminalCommands}
        welcomeMessage="Welcome!"
        promptLabel=">"
      />
    </div>
  );
}

const breakpointEffect = StateEffect.define<{ pos: number, on: boolean }>({
  map: (val, mapping) => ({ pos: mapping.mapPos(val.pos), on: val.on }),
});

const breakpointState = StateField.define<RangeSet<GutterMarker>>({
  create() {
    return RangeSet.empty;
  },
  update(set, transaction) {
    set = set.map(transaction.changes);
    for (let e of transaction.effects) {
      if (e.is(breakpointEffect)) {
        if (e.value.on) {
          set = set.update({ add: [breakpointMarker.range(e.value.pos)] });
        } else {
          set = set.update({ filter: from => from !== e.value.pos });
        }
      }
    }
    return set;
  },
})

function toggleBreakpoint(view: EditorView, pos: number) {
  let breakpoints = view.state.field(breakpointState);
  let hasBreakpoint = false;
  breakpoints.between(pos, pos, () => {
    hasBreakpoint = true;
  });
  view.dispatch({
    effects: breakpointEffect.of({
      pos,
      on: !hasBreakpoint,
    })
  });
}

const breakpointMarker = new class extends GutterMarker {
  toDOM() {
    return document.createTextNode("🔴");
  }
};

const breakpointGutter = [
  breakpointState,
  gutter({
    class: "cm-breapoint-gutter w-12",
    markers: v => v.state.field(breakpointState),
    initialSpacer: () => breakpointMarker,
    domEventHandlers: {
      mouseDown(view, line) {
        toggleBreakpoint(view, line.from);
        return true;
      },
    },
  }),
  EditorView.baseTheme({
    ".cm-breakpoint-gutter .cm-gutterElement": {
      color: "red",
      paddingLeft: "5px",
      cursor: "default",
    },
  }),
];

function CodeView() {
  const text = `
import { StrictMode } from 'react'
import { ThemeProvider } from "@/components/theme-provider"

import { createRoot } from 'react-dom/client'
import App from './App.tsx'
import './index.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ThemeProvider defaultTheme="system">
      <App />
    </ThemeProvider>
  </StrictMode>,
)
    `;
  const extensions = useMemo(() => {
    return [
      javascript({ jsx: true, typescript: true }),
      lineNumbers(),
      breakpointGutter,
    ];
  }, []);
  return (
    <div className="flex h-full">
      <ReactCodeMirror
        className="w-full h-full"
        value={text}
        editable={false}
        basicSetup={false}
        extensions={extensions}
        theme={githubDark}
      />
    </div>
  );
}

type GutterProps = {
  text: string;
  children: ReactNode;
};

function Gutter(props: GutterProps) {
  return (
    <>
      <div className="max-w-48">
        <p>Hello world</p>
      </div>
      {props.children}
    </>
  );
}

function BottomPane() {

  return (
    <Tabs defaultValue="variables">
      <TabsList>
        <TabsTrigger value="variables">Variables</TabsTrigger>
        <TabsTrigger value="terminal">Terminal</TabsTrigger>
      </TabsList>
      <TabsContent value="variables"><Variables /></TabsContent>
      <TabsContent value="terminal"><DebugTerminal /></TabsContent>
    </Tabs>
  );
}


function App() {

  return (
    <div className="h-screen p-4">
      <ResizablePanelGroup
        direction="horizontal"
        className="rounded-lg border md:min-w-[450px]"
      >
        <ResizablePanel defaultSize={25} minSize={15} maxSize={40}>
          <ResizablePanelGroup direction="vertical">
            <ResizablePanel defaultSize={50} minSize={10} maxSize={90}>
              <Breakpoints />
            </ResizablePanel>
            <ResizableHandle />
            <ResizablePanel defaultSize={50} minSize={10} maxSize={90}>
              <StackList />
            </ResizablePanel>
            <ResizableHandle />
          </ResizablePanelGroup>
        </ResizablePanel>
        <ResizableHandle />
        <ResizablePanel defaultSize={75}>
          <ResizablePanelGroup direction="vertical">
            <ResizablePanel defaultSize={75} className="flex flex-col">
              <CodeView />
              <Controls />
            </ResizablePanel>
            <ResizableHandle />
            <ResizablePanel defaultSize={25} minSize={15} maxSize={80}>
              <BottomPane />
            </ResizablePanel>
          </ResizablePanelGroup>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  )
}

export default App
