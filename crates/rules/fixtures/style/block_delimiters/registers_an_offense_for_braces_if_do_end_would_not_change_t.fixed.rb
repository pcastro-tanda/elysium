scope :foo, (lambda do |f|
  where(condition: "value")
end)

expect { something }.to(raise_error(ErrorClass) do |error|
  # ...
end)
