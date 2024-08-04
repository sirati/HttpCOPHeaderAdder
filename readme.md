Minimal working project that proxies http:// and ws:// requests from localhost:8081 to localhost:8080
it injects the following headers:
```http
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```
to allow developing PWA and using secure features that require the headers lole SharedArrayBuffer, WebRTC, etc.

Works with [Dioxus](https://github.com/dioxuslabs/dioxus) hot-reloading