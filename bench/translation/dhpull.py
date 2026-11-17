import sys, json, urllib.request, urllib.error, os
repo, tag, out = sys.argv[1], sys.argv[2], sys.argv[3]
tok = json.load(urllib.request.urlopen(f"https://auth.docker.io/token?service=registry.docker.io&scope=repository:{repo}:pull"))["token"]
def req(url, accept=None):
    r = urllib.request.Request(url, headers={"Authorization": f"Bearer {tok}", **({"Accept": accept} if accept else {})})
    return urllib.request.urlopen(r)
import time
for i in range(30):
    try:
        m = json.load(req(f"https://registry-1.docker.io/v2/{repo}/manifests/{tag}", "application/vnd.oci.image.manifest.v1+json,application/vnd.oci.image.index.v1+json,application/vnd.docker.distribution.manifest.v2+json")); break
    except urllib.error.HTTPError as e:
        print("retry", e.code, file=sys.stderr); time.sleep(30)
for l in m.get("layers", []):
    print(l["mediaType"], l["size"], file=sys.stderr)
g = [l for l in m["layers"] if "gguf" in l["mediaType"] or (l.get("annotations",{}).get("org.cncf.model.filepath","").endswith(".gguf") and "mmproj" not in l.get("annotations",{}).get("org.cncf.model.filepath",""))]
assert g, m
l = g[0]
with req(f"https://registry-1.docker.io/v2/{repo}/blobs/{l['digest']}") as r, open(out + ".part", "wb") as f:
    while True:
        b = r.read(1 << 22)
        if not b: break
        f.write(b)
os.rename(out + ".part", out)
print("done", out, l["size"])
