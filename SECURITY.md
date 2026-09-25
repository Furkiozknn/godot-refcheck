# Security policy

## What godot-refcheck touches

**The command line tool** reads files under the project it is pointed at. It
opens no network connection, starts no process, and never runs the engine or
any code in the project. Symbolic links are not followed, so a link inside a
project cannot lead it to read a file outside it.

It writes in exactly three cases, all asked for on the command line:

- `--fix` edits files it found inside the project, starting at the line it
  reports, replacing only the text it prints (a path, or a byte-order mark). Nothing is
  created or deleted. `--fix-dry-run` shows the same list and writes nothing.
- `--sarif <file>` and `--write-baseline <file>` write the file you name.

**The GitHub Action** (`action.yml`) additionally:

- downloads the release archive for the runner's platform from this
  repository's GitHub Releases, together with the `.sha256` published next to
  it, and refuses to run a binary that does not match. The checksum comes from
  the same release as the archive, so it catches a corrupted or truncated
  download, not a compromised release; pin the Action to a commit SHA if that
  is in your threat model.
- builds from source with `cargo` when no binary is published for the
  platform.
- passes every input to the shell through environment variables, never by
  pasting it into the script, and accepts only `[0-9A-Za-z.+-]` in `version`.
- needs no token and no permissions beyond reading the checked-out code.
  Uploading SARIF is a separate step with `security-events: write`.

## Reporting a vulnerability

Please report privately through
[GitHub Security Advisories](https://github.com/Furkiozknn/godot-refcheck/security/advisories/new)
rather than in a public issue. Especially interesting:

- a project file that makes the tool read or write outside the project, hang,
  or run out of memory
- `--fix` changing anything other than the text it reports
- a way for an Action input or a project file to run a command on the runner

Include the smallest project that shows it, the command line or workflow, and
the version (`godot-refcheck --version`).

Only the latest release is supported.
