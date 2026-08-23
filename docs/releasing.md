# Releasing Kodebar

Kodebar has two independently installable parts:

- the native `kodebar` backend and its systemd user units;
- the Plasma 6 Plasmoid distributed as a `.plasmoid` archive.

The KDE Store artifact contains only the Plasmoid. Its listing must state that
the native backend is required to produce the Snapshot.

## Prepare a release

1. Update `KPlugin.Version` in `frontend/package/metadata.json`.
2. Update the milestone status and user-facing release notes.
3. Run the complete release verification:

   ```bash
   frontend/scripts/verify-release.sh
   ```

4. Build the versioned Store artifact:

   ```bash
   frontend/scripts/package-plasmoid.sh
   ```

   The script prints the resulting path, such as
   `dist/kodebar-0.1.0.plasmoid`. It validates AppStream metadata, creates a
   clean archive rooted at `metadata.json`, and verifies its ZIP structure.

5. Test the exact artifact locally:

   ```bash
   kpackagetool6 --type Plasma/Applet \
     --install dist/kodebar-0.1.0.plasmoid
   ```

   Use `--upgrade` instead of `--install` for an existing 0.1.0-or-newer
   installation.

M2/M3 development packages used the temporary Plasmoid ID `ai.kodebar`.
That ID cannot be upgraded to the Store-valid `io.github.kriss_spy.kodebar`.
Remove the old package, install the release artifact, and add the widget to the
panel again. Plasma does not migrate the old instance or its settings:

```bash
kpackagetool6 --type Plasma/Applet --remove ai.kodebar
```

## KDE Store checklist

- Upload the generated `.plasmoid` file, not the `frontend/` directory.
- Use the metadata name, summary, version, homepage, issue URL, and MIT license.
- State the Plasma 6 minimum and native `kodebar` backend requirement.
- Include screenshots of both the Compact and Full Representations.
- Mention the four current Providers and that ChatGPT concerns subscription
  quota rather than OpenAI API billing.
- Keep `LICENSE`, `NOTICE`, and the Provider SVG files inside the archive.

The package follows KDE's Plasma 6 layout: JSON metadata at the archive root,
`KPackageStructure` set to `Plasma/Applet`, `ui/main.qml` as the implicit entry
point, and `X-Plasma-API-Minimum-Version` set to `6.0`.
