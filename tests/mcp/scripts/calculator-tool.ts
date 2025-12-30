// Name: Calculator Tool
// Description: Perform basic math operations

import "@johnlindquist/kit"

metadata = {
  name: "Math Calculator",
  description: "Perform arithmetic operations on two numbers",
  version: "1.0.0",
}

const { input, output } = defineSchema({
  input: {
    a: {
      type: "number",
      description: "First operand",
      required: true,
    },
    b: {
      type: "number", 
      description: "Second operand",
      required: true,
    },
    operation: {
      type: "string",
      description: "Math operation to perform",
      enum: ["add", "subtract", "multiply", "divide"],
      required: true,
    },
  },
  output: {
    result: {
      type: "number",
      description: "The calculation result",
    },
    expression: {
      type: "string",
      description: "Human-readable expression",
    },
    error: {
      type: "string",
      description: "Error message if operation failed",
    },
  },
} as const)

const { a, b, operation } = await input()

let result: number
let expression: string
let error: string | undefined

const ops: Record<string, { symbol: string; fn: (a: number, b: number) => number }> = {
  add: { symbol: "+", fn: (a, b) => a + b },
  subtract: { symbol: "-", fn: (a, b) => a - b },
  multiply: { symbol: "*", fn: (a, b) => a * b },
  divide: { symbol: "/", fn: (a, b) => a / b },
}

const op = ops[operation]
if (!op) {
  error = `Unknown operation: ${operation}`
  result = NaN
  expression = "ERROR"
} else if (operation === "divide" && b === 0) {
  error = "Division by zero"
  result = NaN
  expression = `${a} / 0 = undefined`
} else {
  result = op.fn(a, b)
  expression = `${a} ${op.symbol} ${b} = ${result}`
}

output({ result, expression, ...(error && { error }) })

if (!metadata.mcp) {
  await div(`<div class="p-8 font-mono text-2xl">${expression}</div>`)
}
