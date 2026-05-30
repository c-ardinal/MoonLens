"""Connect to the live MoonLens lens-frame webview over CDP and dump
the ACTUAL rendered card state + a screenshot. Run with the
browser-use venv python (has `websockets`)."""
import asyncio, json, sys, base64, urllib.request
import websockets

CDP = "http://localhost:9223"

EXTRACT_JS = r"""
(() => {
  const dpr = window.devicePixelRatio;
  const body = document.querySelector('.lens-body') || document.body;
  const cards = Array.from(document.querySelectorAll('.lens-card:not(.lens-card-pending)')).map(card => {
    const p = card.querySelector('.line-text');
    const cs = p ? getComputedStyle(p) : null;
    const cr = card.getBoundingClientRect();
    return {
      text: p ? p.textContent.slice(0, 40) : null,
      cardH: Math.round(cr.height), cardW: Math.round(cr.width),
      font: cs ? cs.fontSize : null,
      weight: cs ? cs.fontWeight : null,
      align: cs ? cs.textAlign : null,
      lineFontSizeVar: card.style.getPropertyValue('--line-font-size'),
      transform: p ? p.style.transform : null,
      ellipsis: p ? p.style.textOverflow : null,
    };
  });
  const pending = document.querySelectorAll('.lens-card-pending').length;
  return JSON.stringify({
    dpr,
    url: location.href,
    visibleCards: cards.length,
    pendingPlaceholders: pending,
    cards,
  });
})()
"""


async def main():
    targets = json.load(urllib.request.urlopen(CDP + "/json"))
    lens = next((t for t in targets if "lens-frame" in t.get("url", "")), None)
    if not lens:
        print("NO lens-frame target found")
        return
    ws_url = lens["webSocketDebuggerUrl"]
    async with websockets.connect(ws_url, max_size=50_000_000) as ws:
        mid = 0
        async def cmd(method, params=None):
            nonlocal mid
            mid += 1
            myid = mid
            await ws.send(json.dumps({"id": myid, "method": method, "params": params or {}}))
            while True:
                msg = json.loads(await ws.recv())
                if msg.get("id") == myid:
                    return msg

        await cmd("Runtime.enable")
        r = await cmd("Runtime.evaluate", {"expression": EXTRACT_JS, "returnByValue": True})
        val = r.get("result", {}).get("result", {}).get("value")
        print("=== RENDER STATE ===")
        print(val)

        shot = await cmd("Page.captureScreenshot", {"format": "png"})
        data = shot.get("result", {}).get("data")
        if data:
            with open("tests/visual/shots/live_lens.png", "wb") as f:
                f.write(base64.b64decode(data))
            print("screenshot saved: tests/visual/shots/live_lens.png")
        else:
            print("screenshot failed:", json.dumps(shot)[:300])


asyncio.run(main())
