/// Dashboard HTML page.
pub fn dashboard_html(version: &str) -> String {
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
            --amber: #f59e0b;
            --amber-bg: rgba(245, 158, 11, 0.1);
            --cyan: #06b6d4;
            --cyan-bg: rgba(6, 182, 212, 0.1);
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

        button.power-off { border-left: 2px solid var(--cyan); }
        button.power-off:hover { border-color: var(--cyan); }

        button.shutdown { border-left: 2px solid var(--red); }
        button.shutdown:hover { border-color: var(--red); }

        button.reset { border-left: 2px solid var(--amber); }
        button.reset:hover { border-color: var(--amber); }

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
            <button class="power-on" id="btn-power-on" onclick="sendAction('power-on')">
                <span class="icon">🟢</span>
                <span class="label">Power On</span>
                <span class="duration">0.5s</span>
            </button>
            <button class="power-off" id="btn-power-off" onclick="sendAction('power-off')">
                <span class="icon">💤</span>
                <span class="label">Power Off</span>
                <span class="duration">0.5s</span>
            </button>
            <button class="shutdown" id="btn-shutdown" onclick="sendAction('shutdown')">
                <span class="icon">🔴</span>
                <span class="label">Shutdown</span>
                <span class="duration">5s</span>
            </button>
            <button class="reset" id="btn-reset" onclick="sendAction('reset')">
                <span class="icon">🔄</span>
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
        const dot = document.getElementById('status-dot');
        const text = document.getElementById('status-text');
        const logEl = document.getElementById('log');
        const btnPowerOn = document.getElementById('btn-power-on');
        const btnPowerOff = document.getElementById('btn-power-off');
        const btnShutdown = document.getElementById('btn-shutdown');
        const btnReset = document.getElementById('btn-reset');
        const allButtons = [btnPowerOn, btnPowerOff, btnShutdown, btnReset];

        const confirmations = {
            'power-off': 'Send graceful shutdown signal to the device?\n\nAll running processes will be stopped and the device will shut down safely.',
            'shutdown': 'Force shutdown the device?\n\nThis will cut power immediately. Any unsaved data will be lost.',
            'reset': 'Hard reset the device?\n\nThis will cut power for 5 seconds, wait 2 seconds, then power on again.',
        };

        function setBusy() {
            dot.className = 'dot busy';
            text.textContent = 'Busy';
            logEl.innerHTML = '<span class="msg busy">Action in progress…</span>';
            allButtons.forEach(b => b.disabled = true);
        }

        function setPolling() {
            dot.className = 'dot busy';
            text.textContent = 'Verifying…';
            logEl.innerHTML = '<span class="msg busy">Waiting for device to stabilize…</span>';
            allButtons.forEach(b => b.disabled = true);
        }

        async function pollStatus(expectedPowerOn, delayMs, pollIntervalMs, maxAttempts) {
            // Wait for the expected delay before starting to poll
            await new Promise(resolve => setTimeout(resolve, delayMs));

            for (let attempt = 0; attempt < maxAttempts; attempt++) {
                await new Promise(resolve => setTimeout(resolve, pollIntervalMs));
                try {
                    const resp = await fetch('/api/status');
                    const data = await resp.json();
                    if (data.power_on === expectedPowerOn) {
                        return true;
                    }
                } catch {
                    // Connection error, try again
                }
            }
            return false;
        }

        function updateButtons(powerOn) {
            btnPowerOn.disabled = powerOn;
            btnPowerOff.disabled = !powerOn;
            btnShutdown.disabled = !powerOn;
            btnReset.disabled = !powerOn;
        }

        function setOnline(powerOn) {
            dot.className = 'dot success';
            text.textContent = 'Online';
            updateButtons(powerOn);
            logEl.innerHTML = '<span>Waiting for action…</span>';
        }

        function setOffline() {
            dot.className = 'dot idle';
            text.textContent = 'Offline';
            btnPowerOn.disabled = false;
            btnPowerOff.disabled = true;
            btnShutdown.disabled = true;
            btnReset.disabled = true;
        }

        function setError(msg) {
            dot.className = 'dot idle';
            text.textContent = 'Offline';
            logEl.innerHTML = '<span class="msg error">' + msg + '</span>';
            btnPowerOn.disabled = false;
            btnPowerOff.disabled = true;
            btnShutdown.disabled = true;
            btnReset.disabled = true;
        }

        async function sendAction(action) {
            const msg = confirmations[action];
            if (msg && !confirm(msg)) return;

            setBusy();
            try {
                const resp = await fetch('/api/' + action, {
                    method: 'POST',
                });
                const data = await resp.json();
                if (resp.ok) {
                    logEl.innerHTML = '<span class="msg success">' + data.message + '</span>';
                    const expectedPowerOn = action === 'power-on' || action === 'reset';
                    const delayMs = data.expected_delay_ms || 2000;
                    const pollIntervalMs = data.poll_ms || 2000;

                    // Poll for status to confirm the device actually reached the expected state
                    setPolling();
                    const confirmed = await pollStatus(expectedPowerOn, delayMs, pollIntervalMs, 15);

                    if (confirmed) {
                        if (expectedPowerOn) {
                            setOnline(true);
                        } else {
                            setOffline();
                        }
                    } else {
                        // Timeout or mismatch — fall back to optimistic state
                        logEl.innerHTML = '<span class="msg busy">Device state not confirmed — showing optimistic state</span>';
                        if (expectedPowerOn) {
                            setOnline(true);
                        } else {
                            setOffline();
                        }
                    }
                } else {
                    logEl.innerHTML = '<span class="msg error">' + data.message + '</span>';
                    updateButtons(btnPowerOn.disabled);
                }
            } catch (err) {
                setError('Network error');
            }
        }

        // Fetch initial state on page load
        (async () => {
            try {
                const resp = await fetch('/api/health');
                const data = await resp.json();
                if (resp.ok) {
                    if (data.power_on) {
                        setOnline(data.power_on);
                    } else {
                        setOffline();
                    }
                } else {
                    setError('Connection failed');
                }
            } catch {
                setError('Network error');
            }
        })();
    </script>
</body>
</html>"#
        .replace("vVERSION", version)
}
