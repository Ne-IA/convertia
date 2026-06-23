# Privacy

ConvertIA is private by design. Everything happens on your own computer.

## What ConvertIA does (and does not) do

- **Your files never leave your machine.** Conversions run entirely on your computer.
  Nothing you drop into ConvertIA is uploaded, copied to a server, or sent anywhere.
- **No accounts, no sign-in.** You never create an account or log in.
- **No telemetry, no tracking.** ConvertIA collects no analytics and no usage data, and
  has no crash reporter that transmits anything. There is no tracking of any kind.
- **Fully offline.** ConvertIA works completely offline and needs no internet connection
  to convert your files. Every conversion engine ships inside the app — nothing is
  downloaded at run time.
- **No automatic updates, no "phone home."** ConvertIA never checks for updates in the
  background and never contacts any server on its own.

## The only time the network is used

ConvertIA makes **no network connection by itself** — ever. The single exception is
**when you choose to click the link** on the About screen that opens the project's
releases page. That click opens the page in **your normal web browser**; ConvertIA
itself still sends nothing. If you never click it, ConvertIA never touches the network
at all.

You can verify this yourself: run ConvertIA with your computer offline (or watch it with
a firewall or a packet monitor) and convert anything — it works exactly the same and
produces no outgoing network traffic.

## Diagnostic logs stay on your machine

ConvertIA keeps a small local log to help diagnose problems. It lives on your computer
and is **never sent anywhere**. By default it records only structural details (formats,
counts, durations, and an output file's name) — never your files' contents and never
their full paths. If you turn on verbose mode to capture a bug report, the log
additionally records full file paths and engine command lines — still only on your
machine. You decide whether to attach a log to a report; ConvertIA never sends one for
you. See [`SECURITY.md`](SECURITY.md) for how to share a log safely.

## The one thing ConvertIA cannot control: cloud-synced folders

ConvertIA saves each result **next to the original file** by default. If that file sits
inside a folder your computer syncs to the cloud — for example **OneDrive**, **iCloud
Drive**, **Dropbox**, or a corporate network share — then **your own sync client** may
upload both the original and the converted result to that service, exactly as it would
for any other file in that folder.

That is your sync software doing its normal job, not ConvertIA. ConvertIA does not cause,
prevent, or detect it. If you want to keep a conversion entirely off the cloud, save it
to — or move it into — a folder that is not synced.
