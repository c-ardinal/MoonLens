"""Directly exercise the running app's translate pipeline via CDP.
Calls the real `run_translate` Tauri command on the main webview
(which has a genuine __TAURI_INTERNALS__.invoke), so this goes
through RoutedTranslator -> Compact -> llm-jp-3-980m, exactly the
lens path. Decides whether the Compact tier echoes English or
translates to Japanese."""
import asyncio, json, urllib.request
import websockets

CDP = "http://localhost:9223"
TESTS = [
    ("Hello, how are you today?", "en", "ja"),
    ("Please call Stella.", "en", "ja"),
    ("The quick brown fox jumps over the lazy dog.", "en", "ja"),
]


def find_main():
    targets = json.load(urllib.request.urlopen(CDP + "/json"))
    # main window is the bare "/" route
    for t in targets:
        if t.get("url", "").rstrip("/").endswith("1420"):
            return t
    # fallback: any page that's not lens/launcher/etc
    for t in targets:
        if t.get("type") == "page":
            return t
    return None


async def main():
    tgt = find_main()
    print("using target:", tgt.get("url"))
    async with websockets.connect(tgt["webSocketDebuggerUrl"], max_size=50_000_000) as ws:
        mid = 0
        async def cmd(method, params=None):
            nonlocal mid
            mid += 1
            myid = mid
            await ws.send(json.dumps({"id": myid, "method": method, "params": params or {}}))
            while True:
                m = json.loads(await ws.recv())
                if m.get("id") == myid:
                    return m
        await cmd("Runtime.enable")
        # sanity: is invoke present?
        chk = await cmd("Runtime.evaluate", {
            "expression": "typeof window.__TAURI_INTERNALS__ + '|' + (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke)",
            "returnByValue": True})
        print("invoke present:", chk.get("result", {}).get("result", {}).get("value"))

        for text, src, tgtl in TESTS:
            expr = (
                "window.__TAURI_INTERNALS__.invoke('run_translate', {args:{"
                f"text:{json.dumps(text)}, source_language:{json.dumps(src)}, target_language:{json.dumps(tgtl)}"
                "}}).then(r => JSON.stringify(r)).catch(e => 'ERR:'+String(e))"
            )
            r = await cmd("Runtime.evaluate", {
                "expression": expr, "awaitPromise": True, "returnByValue": True})
            val = r.get("result", {}).get("result", {}).get("value")
            print(f"\nSRC: {text!r}")
            print(f"OUT: {val}")


asyncio.run(main())
