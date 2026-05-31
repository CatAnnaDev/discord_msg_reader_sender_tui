/**
 * @name DaveSniffer
 * @displayName DaveSniffer
 * @authorId 204972632863539201
 * @invite AnnaDev
 * @version 1.0
 */

module.exports = class DaveSniffer {
    constructor() {
        this._buf = [];
        this._OrigWS = null;
        this._patched = new WeakSet();
    }

    _ts() {
        const t = performance.timeOrigin + performance.now();
        const d = new Date(t);
        return d.toISOString().replace("T", " ").replace("Z", "") +
            ` (${(t / 1000).toFixed(6)})`;
    }

    _hex(buf) {
        const u8 = buf instanceof Uint8Array ? buf : new Uint8Array(buf);
        let s = "";
        for (let i = 0; i < u8.length; i++) s += u8[i].toString(16).padStart(2, "0");
        return s;
    }

    _push(line) {
        this._buf.push(line);
        try { console.log("%c[DaveSniffer]", "color:#0af", line); } catch (e) {}
    }

    _copy(text) {
        try {
            if (window.DiscordNative && DiscordNative.clipboard) {
                DiscordNative.clipboard.copy(text);
                return "DiscordNative.clipboard";
            }
        } catch (e) {}
        try {
            if (window.require) {
                const { clipboard } = window.require("electron");
                clipboard.writeText(text);
                return "electron.clipboard";
            }
        } catch (e) {}
        try {
            navigator.clipboard.writeText(text);
            return "navigator.clipboard";
        } catch (e) {}
        return "FAILED";
    }

    _dump(reason) {
        const text = this._buf.join("\n");
        const how = this._copy(text);
        const msg = `[DaveSniffer] dumped ${this._buf.length} lines (${text.length} chars) to clipboard via ${how} — reason: ${reason}`;
        try { console.log("%c" + msg, "color:#0f0;font-weight:bold"); } catch (e) {}
        try {
            if (window.BdApi && BdApi.showToast) BdApi.showToast(msg, { type: "success", timeout: 8000 });
        } catch (e) {}
    }

    _isVoice(url) {
        return typeof url === "string" && url.includes("discord.media");
    }

    _describe(data, dir, url) {
        const ts = this._ts();
        if (typeof data === "string") {
            let pretty = data, op = "?";
            try { const j = JSON.parse(data); op = j.op; pretty = JSON.stringify(j); } catch (e) {}
            try {
                const j = JSON.parse(data);
                if (j && j.op === 2 && j.d) {
                    this._lastReady = {
                        ssrc: j.d.ssrc,
                        ip: j.d.ip,
                        port: j.d.port,
                    };
                }
                if (j && j.op === 4 && j.d) {
                    const r = this._lastReady || {};
                    const line =
                        "\n================ VOICE TRANSPORT KEY ================\n" +
                        ` mode=${j.d.mode} dave=${j.d.dave_protocol_version}\n` +
                        ` our_ssrc=${r.ssrc} voice_server=${r.ip}:${r.port}\n` +
                        ` secret_key=[${(j.d.secret_key || []).join(",")}]\n` +
                        "=====================================================";
                    this._push(line);
                }
            } catch (e) {}
            this._push(`${ts} ${dir} [text op=${op}]\n  ${pretty}`);
            return;
        }
        const handleBin = (ab) => {
            const u8 = new Uint8Array(ab);
            const b0 = u8[0];
            let op, seq = null;
            if (b0 >= 21 && b0 <= 31) { op = b0; }
            else { seq = (u8[0] << 8) | u8[1]; op = u8[2]; }
            this._push(`${ts} ${dir} [binary ${u8.length}B op=${op} seq=${seq}]\n  ${this._hex(u8)}`);
        };
        if (data instanceof ArrayBuffer) return handleBin(data);
        if (ArrayBuffer.isView(data)) return handleBin(data.buffer);
        if (data instanceof Blob) { data.arrayBuffer().then(handleBin).catch(() => {}); return; }
        this._push(`${ts} ${dir} [binary ?type ${Object.prototype.toString.call(data)}]`);
    }

    _hexAny(v, depth) {
        depth = depth || 0;
        try {
            if (v == null) return String(v);
            if (v instanceof ArrayBuffer) return "ab:" + this._hex(new Uint8Array(v));
            if (ArrayBuffer.isView(v)) return v.constructor.name + ":" + this._hex(new Uint8Array(v.buffer, v.byteOffset, v.byteLength));
            if (Array.isArray(v)) {
                if (v.length && v.length <= 64 && v.every((x) => Number.isInteger(x) && x >= 0 && x <= 255)) {
                    return "arr:" + this._hex(new Uint8Array(v));
                }
                if (depth > 1) return "[array len=" + v.length + "]";
                return "[" + v.slice(0, 8).map((x) => this._hexAny(x, depth + 1)).join(",") + "]";
            }
            if (typeof v === "bigint") return "bi:" + v.toString();
            if (typeof v === "string") return v.length > 80 ? v.slice(0, 80) + "…" : v;
            if (typeof v === "number" || typeof v === "boolean") return String(v);
            if (typeof v === "object") {
                if (depth > 1) return "{obj}";
                const keys = Object.keys(v).slice(0, 12);
                return "{" + keys.map((k) => k + "=" + this._hexAny(v[k], depth + 1)).join(",") + "}";
            }
            return typeof v;
        } catch (e) {
            return "?";
        }
    }

    _patchFn(owner, key, label) {
        const self = this;
        try {
            const orig = owner[key];
            if (typeof orig !== "function" || orig.__daveHooked) return false;
            const wrapped = function (...args) {
                let ret;
                let threw;
                try {
                    ret = orig.apply(this, args);
                } catch (e) {
                    threw = e;
                }
                try {
                    const a = args.map((x) => self._hexAny(x)).join(" | ");
                    let r;
                    if (ret && typeof ret.then === "function") {
                        r = "<promise>";
                        ret.then((val) => {
                            try { self._push(`${self._ts()} DAVECALL ${label}.${key} => (async) ${self._hexAny(val)}`); } catch (e) {}
                        }).catch(() => {});
                    } else {
                        r = self._hexAny(ret);
                    }
                    self._push(`${self._ts()} DAVECALL ${label}.${key}(${a}) => ${r}`);
                } catch (e) {}
                if (threw) throw threw;
                return ret;
            };
            wrapped.__daveHooked = true;
            owner[key] = wrapped;
            this._hookedList.push([owner, key, orig]);
            return true;
        } catch (e) {
            return false;
        }
    }

    _hookDave() {
        const self = this;
        const RE = /dave|mls|ratchet|secret|secureframe|keypackage|verif|fingerprint|sender.?key|epoch|exporter|transition/i;
        const seen = new WeakSet();
        const scanObj = (obj, label) => {
            if (!obj || (typeof obj !== "object" && typeof obj !== "function") || seen.has(obj)) return;
            seen.add(obj);
            let n = 0;
            for (const proto of [obj, obj.prototype]) {
                if (!proto) continue;
                let names = [];
                try { names = Object.getOwnPropertyNames(proto); } catch (e) {}
                for (const k of names) {
                    if (k === "constructor") continue;
                    let isFn = false;
                    try { isFn = typeof proto[k] === "function"; } catch (e) {}
                    if (isFn && RE.test(k)) {
                        if (self._patchFn(proto, k, label)) n++;
                    }
                }
            }
            return n;
        };
        let req;
        try {
            const id = "dave_probe_" + Date.now();
            window.webpackChunkdiscord_app.push([[id], {}, (r) => { req = r; }]);
        } catch (e) {}
        let count = 0;
        let mods = 0;
        try {
            const cache = req && req.c ? req.c : null;
            if (cache) {
                for (const mid of Object.keys(cache)) {
                    const m = cache[mid] && cache[mid].exports;
                    if (!m) continue;
                    mods++;
                    try {
                        count += scanObj(m, "m" + mid) || 0;
                        if (m.default) count += scanObj(m.default, "m" + mid + ".default") || 0;
                        if (m.Z) count += scanObj(m.Z, "m" + mid + ".Z") || 0;
                        if (m.ZP) count += scanObj(m.ZP, "m" + mid + ".ZP") || 0;
                    } catch (e) {}
                }
            }
        } catch (e) {}
        this._push(`${this._ts()} ==== DAVE hook scan: modules=${mods} patched_fns=${count} ====`);
    }

    start() {
        const self = this;
        this._buf = [];
        this._hookedList = [];
        this._push(`==== DaveSniffer ${this._ts()} — capturing. Leave a voice channel to auto-copy, or run window.__daveDump() ====`);

        window.__daveDump = () => self._dump("manual");
        window.__daveScan = () => { try { self._hookDave(); } catch (e) {} };
        setTimeout(() => { try { self._hookDave(); } catch (e) {} }, 4000);
        setTimeout(() => { try { self._hookDave(); } catch (e) {} }, 15000);

        const WS = window.WebSocket;
        this._OrigWS = WS;
        const Wrapped = function (url, protocols) {
            const ws = protocols === undefined ? new WS(url) : new WS(url, protocols);
            try {
                if (self._isVoice(url) && !self._patched.has(ws)) {
                    self._patched.add(ws);
                    self._push(`${self._ts()} ==== NEW VOICE WS ${url} ====`);
                    const origSend = ws.send.bind(ws);
                    ws.send = function (data) {
                        try { self._describe(data, "SEND", url); } catch (e) {}
                        return origSend(data);
                    };
                    ws.addEventListener("message", (ev) => {
                        try { self._describe(ev.data, "RECV", url); } catch (e) {}
                    });
                    ws.addEventListener("close", (ev) => {
                        self._push(`${self._ts()} ==== VOICE WS CLOSE code=${ev.code} reason=${ev.reason} ====`);
                        self._dump("voice ws close");
                    });
                    ws.addEventListener("error", () => {
                        self._push(`${self._ts()} ==== VOICE WS ERROR ====`);
                    });
                }
            } catch (e) {}
            return ws;
        };
        Wrapped.prototype = WS.prototype;
        Wrapped.CONNECTING = WS.CONNECTING;
        Wrapped.OPEN = WS.OPEN;
        Wrapped.CLOSING = WS.CLOSING;
        Wrapped.CLOSED = WS.CLOSED;
        window.WebSocket = Wrapped;
    }

    stop() {
        if (this._OrigWS) window.WebSocket = this._OrigWS;
        try {
            for (const [owner, key, orig] of this._hookedList || []) {
                try { owner[key] = orig; } catch (e) {}
            }
            this._hookedList = [];
        } catch (e) {}
        try { delete window.__daveDump; } catch (e) {}
        try { delete window.__daveScan; } catch (e) {}
        this._push(`${this._ts()} ==== DaveSniffer stopped ====`);
    }
};
