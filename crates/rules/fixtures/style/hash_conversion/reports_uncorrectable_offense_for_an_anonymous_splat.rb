def foo(*)
  Hash[*]
  ^^^^^^^ Prefer `array_of_pairs.to_h` to `Hash[*array]`.
end
