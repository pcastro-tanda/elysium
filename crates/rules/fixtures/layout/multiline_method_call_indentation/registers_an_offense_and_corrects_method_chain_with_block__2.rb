add_to_git_repo(
  initial_repo,
  "about.json" =>
    JSON
        .parse(about_json(about_url: "https://updated.site.com"))
        ^^^^^^ Use 2 (not 4) spaces for indenting an expression spanning multiple lines.
        .tap { |h| h[:component] = true }
        ^^^^ Use 2 (not 4) spaces for indenting an expression spanning multiple lines.
        .to_json,
        ^^^^^^^^ Use 2 (not 4) spaces for indenting an expression spanning multiple lines.
)
