add_to_git_repo(
  initial_repo,
  "about.json" =>
    JSON
        .parse(about_json(about_url: "https://updated.site.com"))
        ^^^^^^ Indent `.parse` 2 spaces more than `JSON` on line 4.
        .tap { |h| h[:component] = true }
        ^^^^ Indent `.tap` 2 spaces more than `JSON` on line 4.
        .to_json,
        ^^^^^^^^ Indent `.to_json` 2 spaces more than `JSON` on line 4.
)
