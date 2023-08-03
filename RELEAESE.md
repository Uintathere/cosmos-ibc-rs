## Releases

Our release process is as follows:

1. In a new branch `release/vX.Y.Z`, update the [changelog](#changelog) to reflect and summarize all changes in
   the release. This involves:
   1. Running `unclog build -u` and copy pasting the output at the top
      of the `CHANGELOG.md` file, making sure to update the header with
      the new version.
   2. Running `unclog release --editor <editor> --version vX.Y.Z` to create a summary of all of the changes
      in this release.
      1. Your text editor will open. Write the release summary, and close the editor.
         1. Make sure to include a comment on whether or not the release contains consensus-breaking changes.
      2. Add this same summary to `CHANGELOG.md` as well.
   3. Committing the updated `CHANGELOG.md` file and `.changelog` directory to the repo.
2. Push this to a branch `release/vX.Y.Z` according to the version number of
   the anticipated release (e.g. `release/v0.18.0`) and open a **draft PR**.
3. If there were changes in the `ibc-derive` crate, we need to publish a new version of that crate.
   1. bump the version in `crates/ibc-derive/Cargo.toml`
   2. Publish `ibc-derive` with `cargo publish -p ibc-derive`
4. Bump all relevant versions in `crates/ibc/Cargo.toml` to the new version and
      push these changes to the release PR.
      + If you released a new version of `ibc-derive` in step 3, make sure to update that dependency.
5. Run `cargo doc -p ibc --all-features --open` locally to double-check that all the
   documentation compiles and seems up-to-date and coherent. Fix any potential
   issues here and push them to the release PR.
6. Run `cargo publish -p ibc --dry-run` to double-check that publishing will work. Fix
   any potential issues here and push them to the release PR.
7. Mark the PR as **Ready for Review** and incorporate feedback on the release.
8. Once approved, merge the PR, and pull the `main` branch.
9. Run `cargo publish -p ibc`
10. Create a signed tag and push it to GitHub: `git tag -s -a vX.Y.Z`. In the tag
   message, write the version and the link to the corresponding section of the
   changelog + Push the tag with `git push --tags`
11. Once the tag is pushed, create a GitHub release and append
   `[📖CHANGELOG](https://github.com/cosmos/ibc-rs/blob/main/CHANGELOG.md#vXYZ)` 
   to the release description.
12. All done! 🎉