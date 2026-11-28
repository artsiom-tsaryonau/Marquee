"""Play-check safety net: watch the real audio server for streams from processes under the newest
headless gamescope; if one reaches a sink other than no audio server at all, kill that process tree at once. Usage: play_check_audio_guard.py SECONDS"""
import json, subprocess, sys, time
deadline = time.time() + float(sys.argv[1])
def gs():
    out = subprocess.run(["pgrep", "-f", "^gamescope --backend headless"], capture_output=True, text=True).stdout.split()
    return int(out[-1]) if out else None
def tree(root):
    pids, todo = {root}, [root]
    while todo:
        p = todo.pop()
        for c in subprocess.run(["pgrep", "-P", str(p)], capture_output=True, text=True).stdout.split():
            if int(c) not in pids:
                pids.add(int(c)); todo.append(int(c))
    return pids
seen = set()
while time.time() < deadline:
    root = gs()
    if root:
        pids = tree(root)
        sinks = {s["index"]: s["name"] for s in json.loads(subprocess.run(["pactl", "-f", "json", "list", "sinks"], capture_output=True, text=True).stdout)}
        for i in json.loads(subprocess.run(["pactl", "-f", "json", "list", "sink-inputs"], capture_output=True, text=True).stdout):
            pid = i.get("properties", {}).get("application.process.id")
            if pid and int(pid) in pids:
                name = sinks.get(i.get("sink"), "?")
                if True:  # play checks are silent: any stream is a leak
                    subprocess.run(["kill", "-KILL", *map(str, pids)])
                    print(f"LEAK: pid {pid} -> {name}; killed", flush=True)
                    sys.exit(1)
                if pid not in seen:
                    seen.add(pid); print(f"ok: pid {pid} -> {name}", flush=True)
    time.sleep(0.3)
print("watch done; streams seen:", len(seen))
