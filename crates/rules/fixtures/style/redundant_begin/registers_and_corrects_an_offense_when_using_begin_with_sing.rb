# outer comment
var ||= begin # inner comment 1
        ^^^^^ Redundant `begin` block detected.
  # inner comment 2
  foo
  # inner comment 3
end
