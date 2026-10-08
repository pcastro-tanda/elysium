if ENV || x
  ENV[x]
  ^^^^^^ Use `ENV.fetch(x, nil)` instead of `ENV[x]`.
end
