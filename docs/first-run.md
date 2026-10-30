# First-run help

**The Assistant can't connect.** Check that Pilot is still running in its terminal, and
that the agent's base URL is `http://127.0.0.1:11500/v1`. Opening
<http://127.0.0.1:11500/v1/models> in a browser should list your model.

**"Model not found" or an empty reply.** The agent's model must match a model your
server has. For Ollama, `ollama list` shows the names; use one exactly, such as
`gemma4:e4b`.

**Port 11500 is in use.** Start Pilot on another port, and use the same port in the
agent's base URL:

```sh
dutchman pilot model --upstream http://localhost:11434/v1 --listen 127.0.0.1:11501
```

**The first answer is slow.** The model server loads the model on the first request.
Later requests are faster.

**A reply starts with "Exact match (no model):".** Pilot read the whole request itself and
proposed the calls without asking the model. SolidCraft still asks you to approve them.

**macOS won't open the download.** Install with Homebrew instead, or approve the binary
in System Settings, Privacy & Security. See [Install](install.md).

**Something else went wrong.** Type `/report` in the Assistant. Pilot writes a redacted
report and a prefilled issue link; you review it before anything is sent. Questions are
welcome on the [Dutchman CAD Discord](https://discord.gg/yTcseBEdeu).
