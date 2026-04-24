/// Dashboard HTML page.
///
/// `token` — API authorization token. If empty, no auth header is sent.
pub fn dashboard_html(version: &str, token: &str) -> String {
    // language=html
    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>🖲️ Panther Minor Controller</title>
    <style>
        *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }

        :root {
            --bg: #09090b;
            --surface: #111113;
            --surface-hover: #18181b;
            --border: #222224;
            --border-hover: #333336;
            --text: #fafafa;
            --text-secondary: #a1a1aa;
            --text-muted: #52525b;
            --green: #22c55e;
            --green-bg: rgba(34, 197, 94, 0.1);
            --red: #ef4444;
            --red-bg: rgba(239, 68, 68, 0.1);
            --yellow: #eab308;
            --yellow-bg: rgba(234, 179, 8, 0.1);
            --blue: #3b82f6;
            --blue-bg: rgba(59, 130, 246, 0.1);
            --radius: 12px;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
            background: var(--bg);
            color: var(--text);
            min-height: 100vh;
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            gap: 1.5rem;
            padding: 2rem;
        }

        .card {
            background: var(--surface);
            border: 1px solid var(--border);
            border-radius: 16px;
            padding: 2.5rem;
            width: 100%;
            max-width: 420px;
            box-shadow: 0 0 0 1px rgba(255,255,255,0.02), 0 20px 60px -15px rgba(0,0,0,0.5);
        }

        .header {
            text-align: center;
            margin-bottom: 2rem;
        }

        .header h1 {
            font-size: 1.25rem;
            font-weight: 600;
            letter-spacing: -0.025em;
            color: var(--text);
            margin-bottom: 0.25rem;
        }

        .header .subtitle {
            font-size: 1.1rem;
            color: var(--text-secondary);
            display: block;
            font-weight: 400;
            letter-spacing: 0.04em;
        }

        .status-bar {
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 0.5rem;
            padding: 0.6rem 1rem;
            background: var(--bg);
            border: 1px solid var(--border);
            border-radius: 999px;
            font-size: 0.75rem;
            font-weight: 500;
            color: var(--text-secondary);
            margin-bottom: 1.75rem;
            transition: all 0.3s ease;
        }

        .status-bar .dot {
            width: 7px;
            height: 7px;
            border-radius: 50%;
            background: var(--text-muted);
            transition: background 0.3s ease, box-shadow 0.3s ease;
        }

        .status-bar .dot.idle {
            background: var(--text-muted);
            box-shadow: none;
        }
        .status-bar .dot.busy {
            background: var(--yellow);
            box-shadow: 0 0 8px rgba(234, 179, 8, 0.4);
            animation: pulse 1s ease-in-out infinite;
        }
        .status-bar .dot.success {
            background: var(--green);
            box-shadow: 0 0 8px rgba(34, 197, 94, 0.4);
        }
        .status-bar .dot.error {
            background: var(--red);
            box-shadow: 0 0 8px rgba(239, 68, 68, 0.4);
        }

        @keyframes pulse {
            0%, 100% { opacity: 1; }
            50% { opacity: 0.5; }
        }

        .actions {
            display: flex;
            flex-direction: column;
            gap: 0.5rem;
            margin-bottom: 1.5rem;
        }

        button {
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 0.6rem;
            padding: 0.8rem 1rem;
            border: 1px solid var(--border);
            border-radius: var(--radius);
            background: var(--surface);
            color: var(--text);
            font-size: 0.85rem;
            font-weight: 500;
            font-family: inherit;
            cursor: pointer;
            transition: all 0.15s ease;
            position: relative;
            overflow: hidden;
        }

        button:hover {
            background: var(--surface-hover);
            border-color: var(--border-hover);
        }

        button:active {
            transform: scale(0.985);
        }

        button:disabled {
            opacity: 0.5;
            cursor: not-allowed;
            transform: none;
        }

        button .icon {
            width: 18px;
            height: 18px;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 0.9rem;
        }

        button .label {
            flex: 1;
            text-align: left;
        }

        button .duration {
            font-size: 0.7rem;
            color: var(--text-muted);
            font-weight: 400;
        }

        button.power-on { border-left: 2px solid var(--green); }
        button.power-on:hover { border-color: var(--green); }

        button.power-off { border-left: 2px solid var(--red); }
        button.power-off:hover { border-color: var(--red); }

        button.reset { border-left: 2px solid var(--yellow); }
        button.reset:hover { border-color: var(--yellow); }

        .log {
            background: var(--bg);
            border: 1px solid var(--border);
            border-radius: var(--radius);
            padding: 0.75rem 1rem;
            font-size: 0.75rem;
            font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
            color: var(--text-muted);
            min-height: 2.5rem;
            display: flex;
            align-items: center;
            transition: color 0.3s ease;
        }

        .log .msg.success { color: var(--green); }
        .log .msg.error { color: var(--red); }
        .log .msg.busy { color: var(--yellow); }

        .footer {
            text-align: center;
            margin-top: 1.5rem;
            font-size: 0.65rem;
            color: var(--text-muted);
            letter-spacing: 0.02em;
        }
    </style>
</head>
<body>
    <div class="card">
        <div class="header">
            <h1>🖲️<br/>Panther Minor<small class="subtitle">Controller</small></h1>
        </div>

        <div class="status-bar">
            <span class="dot idle" id="status-dot"></span>
            <span id="status-text">Offline</span>
        </div>

        <div class="actions">
            <button class="power-on" onclick="sendAction('power-on')">
                <span class="icon">⏻</span>
                <span class="label">Power On</span>
                <span class="duration">0.5s</span>
            </button>
            <button class="power-off" onclick="sendAction('power-off')">
                <span class="icon">⏻</span>
                <span class="label">Power Off</span>
                <span class="duration">5s</span>
            </button>
            <button class="reset" onclick="sendAction('reset')">
                <span class="icon">↺</span>
                <span class="label">Hard Reset</span>
                <span class="duration">7s</span>
            </button>
        </div>

        <div class="log" id="log">
            <span>Waiting for action…</span>
        </div>
    </div>
    <div class="footer">panther-minor-controller vVERSION</div>

    <script>
        const API_TOKEN = 'API_TOKEN_VALUE';
        const dot = document.getElementById('status-dot');
        const text = document.getElementById('status-text');
        const logEl = document.getElementById('log');
        const buttons = document.querySelectorAll('button');

        function setBusy() {
            dot.className = 'dot busy';
            text.textContent = 'Busy';
            logEl.innerHTML = '<span class="msg busy">Action in progress…</span>';
            buttons.forEach(b => b.disabled = true);
        }

        function setSuccess() {
            dot.className = 'dot success';
            text.textContent = 'Online';
            buttons.forEach(b => b.disabled = false);
        }

        function setError(msg) {
            dot.className = 'dot idle';
            text.textContent = 'Offline';
            logEl.innerHTML = '<span class="msg error">' + msg + '</span>';
            buttons.forEach(b => b.disabled = false);
        }

        function setReady() {
            dot.className = 'dot success';
            text.textContent = 'Online';
        }

        async function sendAction(action) {
            setBusy();
            try {
                const resp = await fetch('/api/' + action, {
                    method: 'POST',
                    headers: API_TOKEN ? { 'Authorization': 'Bearer ' + API_TOKEN } : {},
                });
                const data = await resp.json();
                if (resp.ok) {
                    logEl.innerHTML = '<span class="msg success">' + data.message + '</span>';
                    setSuccess();
                } else {
                    setError(data.error || 'Unknown error');
                }
            } catch (err) {
                setError('Connection failed');
            }
            setTimeout(setReady);
        }
    </script>
</body>
</html>"#
        .replace("vVERSION", version)
        .replace(
            "API_TOKEN_VALUE",
            &token.replace('\\', "\\\\").replace('\'', "\\'"),
        )
}
