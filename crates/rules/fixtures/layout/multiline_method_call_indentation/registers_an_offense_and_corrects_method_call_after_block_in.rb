add_to_git_repo(
  initial_repo,
  "about.json" =>
    JSON
      .parse(about_json(about_url: "https://updated.site.com"))
      ^^^^^^ Align `.parse` with `JSON` on line 4.
      .tap { |h| h[:component] = true }
      ^^^^ Align `.tap` with `JSON` on line 4.
      .to_json,
      ^^^^^^^^ Align `.to_json` with `JSON` on line 4.
)
