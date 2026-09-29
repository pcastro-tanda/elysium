scope :foo, (lambda { |f|
                    ^ Avoid using `{...}` for multi-line blocks.
  where(condition: "value")
})

expect { something }.to(raise_error(ErrorClass) { |error|
                                                ^ Avoid using `{...}` for multi-line blocks.
  # ...
})
