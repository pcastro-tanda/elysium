@foo ||= begin
  @bar ||= begin
           ^^^^^ Redundant `begin` block detected.
    baz
  end
end
