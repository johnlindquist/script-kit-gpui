// Name: Demo Arg to Div

// Import SDK to register globals (no named imports needed)
import './kit-sdk';

const fruit = await arg('Pick a fruit', [
  { name: 'Apple', value: 'apple', description: 'A red fruit' },
  { name: 'Banana', value: 'banana', description: 'A yellow fruit' },
  { name: 'Cherry', value: 'cherry', description: 'A small red fruit' },
]);

await div(md(`# You selected

**${fruit}**

Thanks for using Script Kit!`));
