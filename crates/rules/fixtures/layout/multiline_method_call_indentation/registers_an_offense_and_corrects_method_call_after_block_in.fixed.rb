add_to_git_repo(
  initial_repo,
  "about.json" =>
    JSON
    .parse(about_json(about_url: "https://updated.site.com"))
    .tap { |h| h[:component] = true }
    .to_json,
)
