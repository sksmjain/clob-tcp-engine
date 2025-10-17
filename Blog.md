# Why We’re Building Our CLOB on Raw TCP, Not HTTP
When most developers think about building an API service, HTTP is the default choice. Its well understood, easy to debug, and universally supported - browsers, SDKs, curl - everthing speaks HTTP.

So it's fair to ask:
Why are we building our Central Limit Order Book (CLOB) on a custom TCP server instead?

The short answer: performance, determinism and control
The long answer reveals how the design of trading system differs from ordinary web apps.

