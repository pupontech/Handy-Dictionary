# GitHub dictionary sync: user guide

GitHub sync copies **Custom Words and Replacement Rules only** between your computers. Handy talks directly to GitHub over HTTPS. You do not need Git, GitHub Desktop, a local repository, a mapped drive, or the Google Drive client.

Sync is optional. It is **manual in this version**: press **Sync now** on the computer you edited, then press it on your other computer. Handy continues working without internet. There is no background upload, and connecting an account does not upload your dictionary.

## Privacy: what we do and do not store

**The Handy Dictionary maintainers do not operate a sync server and do not receive or store your GitHub password or authorization token.** Your computer communicates with GitHub itself. You sign in on `github.com` in your browser, not in a Handy password form.

It would be misleading to say that no credential is stored anywhere. Staying connected requires a GitHub **authorization token**, which is a credential:

- Handy stores that token locally in **Windows Credential Manager**, **macOS Keychain**, or the Linux desktop's **Secret Service** credential store.
- The token is read by the native Rust backend when a request needs it. It is not returned to the JavaScript/webview interface, put in dictionary exports, put in ordinary settings or sync metadata, or intentionally logged by the sync code.
- There is **no plaintext-file fallback** if the OS credential store is missing, locked, or fails. Unlock/fix the credential store and reconnect instead.
- The temporary device authorization secret stays in native memory. The short code displayed in Handy is only for the browser approval step. Never approve a device code someone else sent you.
- Disconnect deletes the local stored token. Revoking access on GitHub is a separate step described below.

Your **dictionary is data, not a credential vault**. It is stored as readable JSON in your private GitHub repository and in local settings, sync metadata, and recovery snapshots. It is **not end-to-end encrypted** by Handy. GitHub can process the repository content, and anyone you give repository access to can read it. Do not put passwords, API keys, confidential customer material, or other secrets in dictionary entries.

OS credential storage is not a guarantee against malware running as you, an unlocked compromised account, or an administrator. It protects against casually copying a settings/export file to obtain the token. Tokens are necessarily present in process memory while being used.

## Before you start

You need:

1. A GitHub account. Each person normally uses their **own** account and private dictionary repository.
2. A dedicated **private repository**, for example `handy-dictionary-data`.
3. The Handy Dictionary GitHub App installed on **only that repository**.
4. A build with its public GitHub App configuration supplied by the maintainer, or those same public configuration values supplied separately by the maintainer.

You do **not** need a paid GitHub plan just for a private personal repository. GitHub's applicable account limits and policies still apply.

If Handy says this build has no GitHub App configuration, the release maintainer must register/configure the app first. Ordinary users should not need to create their own GitHub App. The **Public GitHub App configuration** section is for the maintainer-supplied Client ID and app slug, not for a password, access token, client secret, or private key. Never enter any of those secrets there. Only use app details from a maintainer/build you trust.

## Step 1: create a private repository on GitHub

1. In Handy, open **Dictionary → GitHub sync** and press **Create private repository**. This opens GitHub's repository creation page; it does not create one automatically.
2. Sign into your own GitHub account in your browser.
3. Choose yourself as owner and a name such as `handy-dictionary-data`.
4. Select **Private**. Handy refuses a repository that GitHub reports as public.
5. **Initialize it with a README** so that it has a default branch. An empty, uninitialized repository cannot be used reliably as file storage.
6. Create the repository. Note its owner and name, for example `alice/handy-dictionary-data`.

Do not use the public `pupontech/Handy-Dictionary` source-code repository. Do not put the dictionary in a work/source-code repository containing other sensitive files.

## Step 2: install the GitHub App

1. Press **Install GitHub App** in Handy.
2. On GitHub, check the app's identity and requested permissions.
3. Choose **Only select repositories** and select your private dictionary repository.
4. Complete installation.

The app needs **repository Contents: read and write**; GitHub also provides basic repository metadata. Contents access is repository-wide, **not restricted to one file** by GitHub. That is why a dedicated repository and selected-repository installation matter. Handy's sync implementation uses only `handy-dictionary.json` in that repository.

An organization may require administrator approval. Start with a repository in your personal account if you do not want organization setup. Never grant access to all repositories just to bypass a setup error.

## Step 3: connect Handy

1. Press **Connect GitHub**.
2. Handy displays a short device code.
3. Press **Open GitHub authorization**. Check that the browser is on `https://github.com/login/device`.
4. Enter **the code shown by your own Handy app**, verify the app identity, and approve the request.
5. Return to Handy and press **Check authorization**. If approval is still pending, wait a few seconds and check again. GitHub requires a minimum delay between checks; Handy enforces it.
6. When connected, enter the **Repository owner** and **Repository name** separately. For `alice/handy-dictionary-data`, enter `alice` and `handy-dictionary-data`, not a URL.
7. Press **Use this private repository**. This verifies repository privacy/access and records its default branch. It does not upload dictionary entries.

Installing the app and authorizing this computer are separate steps. A successful sign-in does not mean the app is installed on the repository you selected.

## Step 4: first sync

**Export a JSON dictionary backup first**, especially if different computers already have different dictionaries.

Read and check the first-sync confirmation, then press **Sync now**. On a newly connected device, Handy combines local entries with existing remote entries. This preserves entries instead of treating an unfamiliar dictionary as an instruction to delete everything missing from it.

- If the remote dictionary does not exist, Handy creates `handy-dictionary.json` on the selected branch.
- If it exists, Handy validates it and combines independent entries.
- If the same replacement has different meanings on the two sides, Handy asks you to resolve the conflict.
- A local recovery snapshot must be saved before either side is changed.
- After uploading, Handy downloads the target file again and verifies the contents before declaring success.

**A first sync cannot know that an entry in an old local dictionary was deliberately deleted elsewhere before this computer ever synchronized.** Such an entry can be added back by the initial combination. For a fresh computer, start with an empty dictionary if you want the remote dictionary to be the starting point. Review the result of the first sync before using dictation.

The checkbox is consent to a two-way first merge, not a “download only” mode.

## Step 5: connect another computer

Install Handy Dictionary on the other computer, repeat app authorization, and select the **same owner/repository**. You do not clone or download a repository yourself. You normally only need to install the GitHub App once per repository/account, but each computer needs its own authorization token.

Check first-sync consent and press **Sync now**. Custom Words and Replacement Rules should now agree. GitHub tokens do not travel with your dictionary or with a portable app folder: authorize each computer/OS account separately.

## Everyday use, offline edits, and deletions

After adding, editing, deleting, or importing dictionary entries, press **Sync now**. On your other computer, press **Sync now** to receive them. Changes stay local until you explicitly synchronize.

After a successful sync, Handy remembers that device's last shared dictionary. It compares that baseline with both the current local dictionary and GitHub's current dictionary:

- Independent additions and edits are merged.
- A deletion on one side is kept when the other side did not change that entry.
- An edit on one side and deletion of the same entry on the other side is a conflict.
- The same replacement changed differently on both sides is a conflict.
- Words and replacement source phrases are matched case-insensitively.
- Editing a word or replacement's source phrase to a different identity is represented as removing the old entry and adding the new one.

Replacement-rule order keeps the current device's existing order, then appends unseen remote entries. Order is not independently synchronized as a user-editable setting.

If another computer writes while yours uploads, GitHub's revision check rejects the stale write rather than blindly overwriting it. Press **Sync now** again. If you edit locally during a network request, Handy refuses to overwrite those edits; the remote write may already have succeeded. Sync again to reconcile. Neither case means you should delete your local dictionary.

## Resolving conflicts

The conflict panel shows each affected entry's **This computer** and **GitHub** value. “Deleted” means the entry is absent on that side.

- **Keep this computer's conflicting entries** chooses this computer's side for **all listed conflicts**.
- **Keep GitHub's conflicting entries** chooses GitHub's side for **all listed conflicts**.
- Either choice still merges independent nonconflicting changes; it does not replace the entire dictionary with one side.
- **Cancel** closes the panel without applying that preview. Edit/export your dictionary or inspect GitHub before syncing again if you want different choices for individual entries.

A conflict choice is valid only for the local dictionary and remote file revision you previewed. If either changes, Handy asks you to sync and review again instead of applying your old choice to unseen changes.

## Signing in again

The app's token policy is controlled by its GitHub App registration. With GitHub's expiring user tokens enabled, a token normally expires after eight hours. You may also need to reconnect after revocation or account changes.

This serverless implementation does **not** embed a GitHub App client secret/private key or silently refresh a token using such a secret. Press **Reconnect GitHub** and repeat browser authorization when required. A “connected” indicator means a token is present locally; it is not proof that GitHub still accepts it. A sync request checks actual access.

## Disconnecting and revoking access

1. In Handy, press **Disconnect**. This removes the stored authorization token for this GitHub App from this OS account's credential store and cancels the pending in-app authorization/preview.
2. To revoke authorization on GitHub, visit **GitHub Settings → Applications → Authorized GitHub Apps**, select this app, and revoke access.
3. To remove repository access too, visit **Installed GitHub Apps**, configure/uninstall the app, or remove the selected repository from its access list.
4. Repeat local Disconnect on other computers as needed.

Disconnect **does not** delete dictionary entries, your GitHub repository, local backups, or the merge baseline. Keeping the baseline makes reconnection to the same repository safer for deletions. Other Handy installations using the same OS account and GitHub App Client ID share that credential-store entry: disconnecting one affects the others. Deleting a portable folder or uninstalling the app alone is not a reliable token-revocation method.

## Recovery and changing repositories

Handy retains up to **twenty pre-sync JSON snapshots** under `dictionary-sync/backups` inside the app data directory. Installed copies use Handy Dictionary's app-data location; portable copies use the `Data` folder alongside the executable. Use the existing app-data folder action in Handy to locate it. Sort snapshots by modification time and copy a snapshot somewhere safe before experimenting.

Snapshots use the existing dictionary JSON format. Import merges entries; it is **not** an exact rollback of deletions. For an exact local restoration, disconnect, export the current dictionary, clear the current Custom Words/Replacement Rules through Handy, and import the chosen snapshot. Before reconnecting, consider whether GitHub's version also needs recovery, otherwise a later merge may reintroduce changes.

The repository's GitHub commit history is another recovery source. In the browser, open `handy-dictionary.json` and its history to inspect/download an earlier version. Keep a copy of both current dictionaries before restoring remote data. Changing a file on GitHub does not update another computer until it synchronizes.

Selecting a **different repository or branch** starts a new first-sync baseline and requires first-sync consent again. Existing local entries are not erased. If a previously synchronized remote file disappears, Handy stops rather than recreating potentially deleted data automatically. Restore the file from GitHub history, or intentionally select a different private repository and review first-sync behavior.

Deleting a word now does **not** erase it from earlier GitHub commits or retained local snapshots. For sensitive-data removal, follow GitHub's history-removal guidance and remove local copies/backups too; normal sync is not secure erasure.

## Troubleshooting

- **Private repository required:** change visibility to private or select a dedicated private repository. Do not make it public to troubleshoot. Privacy is checked again on each sync, but changing visibility while a request is in flight cannot be made atomic with a file write.
- **Not found / permission denied:** confirm owner/name, app installation on that exact repository, Contents read/write permission, and organization approval. An uninitialized repository needs a README/default branch. Branch protection/rules may prohibit direct commits; use an unprotected dedicated data repository instead of weakening your source repository's protections.
- **Sign in again:** reconnect. GitHub rejected or expired the token. Your local dictionary is still available.
- **Credential store unavailable:** unlock Windows/macOS credentials or start/unlock a Linux Secret Service provider, such as GNOME Keyring or KeePassXC with Secret Service enabled. No token is saved in a text file as a workaround.
- **Authorization pending:** complete the browser step and wait before checking again. An expired/denied code requires starting Connect again.
- **Remote/local changed or preview expired:** sync again and review the fresh result.
- **Malformed/unsupported dictionary:** Handy refuses invalid JSON, unsupported schema versions, duplicate case-insensitive entries, blank replacements, overlong words, and oversized files. Correct or recover the remote file; it will not partially import it. The limit is 10,000 total words/rules and 1,000,000 encoded JSON bytes.
- **Local storage/state error:** check disk space and write access. A damaged sync-state file is not silently treated as a new device. Preserve settings, state, and backups before requesting help.
- **Network/rate limit/server error:** wait and retry. This version does not run a background retry loop or promise an automatic delivery time.

When reporting a problem, share the error message and steps, **not your password, token, device code, dictionary contents, or unredacted data files**.

## Technical summary

Only `handy-dictionary.json` is uploaded, with schema `handy-dictionary`, version `1`, `customWords`, and `replacements`. Models, recordings, transcription history, prompts, unrelated app settings, and tokens are not included by the sync serializer. GitHub commits use the generic message “Sync Handy Dictionary”; GitHub may record the authenticated account/app and commit timestamps.

Connections use GitHub's device authorization endpoints and repository Contents API. There is no maintainer-hosted relay, credential collection service, or local Git checkout. GitHub itself still receives authentication requests, repository data, and normal network/account metadata. Other Handy features, such as model downloads or explicitly configured cloud transcription/post-processing, have separate network/privacy behavior; this guide describes dictionary sync only.
