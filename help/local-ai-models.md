---
title: Local AI Models
group: feature
---
# Local AI Models

AI Translation can use a model that runs on your own server instead of a paid, hosted API. This
keeps your content on your own machine and avoids per-request costs, at the expense of needing a
one-time setup on the server itself before SynapCMS can use it.

- [Before you start](#before-you-start)
- [Install and run a local model server](#install-and-run)
- [Add it as a provider in SynapCMS](#add-provider)
- [Troubleshooting](#troubleshooting)

<a id="before-you-start"></a>
## Before you start

This is only available to a **Super Admin** — a local model server is only reachable from the
same machine SynapCMS runs on, so setting one up (and testing it, and translating with it) is
restricted to whoever administers that machine. If you don't see an "OpenAI-Compatible" option
when adding an AI provider, you're signed in as a Site Admin or Editor rather than a Super Admin.

You'll need shell access to the server SynapCMS is installed on — this isn't something you can
finish from the SynapCMS admin alone. If that's not you, ask whoever manages the server to
complete the steps below first, or use a hosted provider (Anthropic or DeepSeek) instead, which
only need an API key.

<a id="install-and-run"></a>
## Install and run a local model server

SynapCMS talks to any server that speaks the OpenAI-compatible chat API — in practice this means
[Ollama](https://ollama.com) (the most common choice) or LM Studio. These steps use Ollama:

1. Install it on the server by following the instructions at [ollama.com](https://ollama.com) for
   your operating system.
2. Pull at least one model. From a terminal on the server:
   ```
   ollama pull llama3
   ```
   Larger, more capable models give better translations but need more RAM/VRAM and take longer to
   respond — start with a mid-sized model and try a bigger one later if quality isn't good enough.
3. Confirm it's running and reachable:
   ```
   curl http://localhost:11434/v1/models
   ```
   This should print a JSON list including the model you just pulled. If it doesn't respond at
   all, Ollama isn't running — see [Troubleshooting](#troubleshooting) below.

<a id="add-provider"></a>
## Add it as a provider in SynapCMS

1. Go to the site's **Settings → AI Translation** tab.
2. Under **Add Provider**, choose **OpenAI-Compatible**.
3. Fill in:
   - **Base URL**: `http://localhost:11434/v1` (include the `/v1` — this is Ollama's default; a
     different local server or a non-default Ollama port will have its own address instead).
   - **API key**: leave this blank. A local server on your own machine doesn't need one.
4. Click **Connect and load models** to confirm SynapCMS can reach it, then pick the model you
   pulled from the dropdown.
5. Save. SynapCMS sends it one test message before marking the provider verified — for a large
   model this can take a while the first time, especially if it isn't already loaded into memory,
   so give it a minute before assuming something's wrong.

Once verified, this provider appears in the Translate dropdown on any post, page, form, or poll
for this site, exactly like a hosted provider would.

<a id="troubleshooting"></a>
## Troubleshooting

**"Connecting and loading models…" never finishes, or fails with a connection error.**
The server isn't running, isn't listening on the address you entered, or a firewall is blocking
it. On the server, check:
```
systemctl status ollama
```
or, if it's not running as a service, start it in a terminal with `ollama serve` and watch for
errors. Also double-check the Base URL matches exactly what `curl` reached in the step above.

**"Provider added, but verification failed."**
The connection worked but the test message itself failed — most often the model name doesn't
exactly match one from `ollama list`, or the model is still loading into memory. Use the **Test**
icon on the saved provider to retry once you've confirmed the model name.

**Translations are slow.**
This is normal for a local model, especially a large one on a server without a dedicated GPU — a
translation can take anywhere from a few seconds to over a minute. This is why AI Translation
shows a spinner and status text while a request is in progress rather than appearing to hang.

**I don't see "OpenAI-Compatible" as an option at all.**
See [Before you start](#before-you-start) above — this option is Super Admin only.
